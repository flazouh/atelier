//! The tracker on real git repositories and real files, through the project, and on a captured run.
use std::{fs, path::Path, process::Command, sync::Arc};

use atelier_agents::{
    claude_code::{ClaudeLineMapper, LineMapper},
    session::{Event, PermissionRequest, ToolCall, ToolId, ToolKind, ToolOutput, ToolStatus},
};
use atelier_project::LocalProject;
use tempfile::TempDir;

use super::TurnTracker;
use crate::{Change, Content, FileReview};

/// A git repository with a first commit, and a project on it.
struct Repo {
    dir: TempDir,
    project: Arc<LocalProject>,
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().expect("git runs");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

impl Repo {
    /// A repository holding `files`, all committed.
    fn with(files: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        git(dir.path(), &["config", "user.email", "t@example.com"]);
        git(dir.path(), &["config", "user.name", "T"]);
        git(dir.path(), &["config", "commit.gpgsign", "false"]);
        for (path, text) in files {
            write(dir.path(), path, text.as_bytes());
        }
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "first"]);
        let project = Arc::new(LocalProject::open(dir.path()).unwrap());
        Self { dir, project }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, path: &str, text: &str) {
        write(self.root(), path, text.as_bytes());
    }

    fn abs(&self, path: &str) -> String {
        self.root().join(path).to_string_lossy().to_string()
    }

    fn begin(&self) -> TurnTracker {
        TurnTracker::begin(self.project.as_ref())
    }

    /// An edit tool announcing `path`, the way a stream does: first the call, then its file.
    fn announce(&self, tracker: &mut TurnTracker, id: &str, kind: ToolKind, path: &str) {
        let call = ToolCall {
            id: ToolId::new(id),
            name: "Tool".into(),
            kind,
            input: serde_json::Value::Null,
            file: None,
            parent: None,
            status: ToolStatus::Running,
        };
        tracker.observe(self.project.as_ref(), &Event::ToolStarted(call));
        tracker.observe(self.project.as_ref(), &Event::ToolTarget { id: ToolId::new(id), file: self.abs(path) });
    }

    fn files(&self, tracker: TurnTracker) -> Vec<FileReview> {
        tracker.finish(self.project.as_ref()).files().to_vec()
    }
}

fn write(root: &Path, path: &str, bytes: &[u8]) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, bytes).unwrap();
}

fn hunks(file: &FileReview) -> Vec<(std::ops::Range<usize>, std::ops::Range<usize>)> {
    match &file.content {
        Content::Text(merged) => merged.hunks().iter().map(|h| (h.removed.clone(), h.added.clone())).collect(),
        other => panic!("{} is {other:?}", file.path),
    }
}

#[test]
fn a_file_an_edit_tool_names_has_its_text_taken_before_the_edit_lands() {
    let repo = Repo::with(&[("a.txt", "one\ntwo\nthree\n")]);
    let mut tracker = repo.begin();
    repo.announce(&mut tracker, "t1", ToolKind::Edit, "a.txt");
    repo.write("a.txt", "one\nTWO\nthree\n");
    let files = repo.files(tracker);
    assert_eq!(files.len(), 1);
    let file = &files[0];
    assert_eq!((file.path.as_str(), &file.change, file.exact), ("a.txt", &Change::Modified, true));
    assert_eq!((file.before.as_deref(), file.after.as_deref()), (Some("one\ntwo\nthree\n"), Some("one\nTWO\nthree\n")));
    assert_eq!(hunks(file), [(1..2, 2..3)]);
    assert_eq!(file.counts(), (1, 1));
}

#[test]
fn the_baseline_is_the_text_at_the_first_touch_and_a_second_edit_does_not_move_it() {
    let repo = Repo::with(&[("a.txt", "1\n2\n3\n")]);
    let mut tracker = repo.begin();
    repo.announce(&mut tracker, "t1", ToolKind::Edit, "a.txt");
    repo.write("a.txt", "1\nX\n3\n");
    repo.announce(&mut tracker, "t2", ToolKind::Edit, "a.txt");
    repo.write("a.txt", "1\nX\nY\n");
    let file = &repo.files(tracker)[0];
    assert_eq!(file.before.as_deref(), Some("1\n2\n3\n"));
    assert_eq!(file.after.as_deref(), Some("1\nX\nY\n"));
}

#[test]
fn the_uncommitted_text_the_user_had_is_the_baseline_not_the_last_commit() {
    let repo = Repo::with(&[("a.txt", "committed\n")]);
    repo.write("a.txt", "the user's own change\n");
    let mut tracker = repo.begin();
    repo.announce(&mut tracker, "t1", ToolKind::Write, "a.txt");
    repo.write("a.txt", "the user's own change\nand the agent's line\n");
    let file = &repo.files(tracker)[0];
    assert_eq!(file.before.as_deref(), Some("the user's own change\n"));
    assert_eq!(hunks(file), [(1..1, 1..2)], "only the agent's line is a hunk");
}

#[test]
fn a_file_the_call_names_only_in_its_permission_question_is_taken_too() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let mut tracker = repo.begin();
    let call = ToolCall {
        id: ToolId::new("t1"),
        name: "Write".into(),
        kind: ToolKind::Write,
        input: serde_json::Value::Null,
        file: Some(repo.abs("a.txt")),
        parent: None,
        status: ToolStatus::Pending,
    };
    let ask = PermissionRequest { id: atelier_agents::session::RequestId::new("r"), call, reason: None, choices: vec![] };
    tracker.observe(repo.project.as_ref(), &Event::Permission(ask));
    repo.write("a.txt", "y\n");
    assert_eq!(repo.files(tracker)[0].before.as_deref(), Some("x\n"));
}

#[test]
fn a_new_file_a_tool_writes_is_added_and_its_baseline_is_nothing() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let mut tracker = repo.begin();
    repo.announce(&mut tracker, "t1", ToolKind::Write, "new/dir/b.txt");
    repo.write("new/dir/b.txt", "hello\nworld\n");
    let file = &repo.files(tracker)[0];
    assert_eq!((file.path.as_str(), &file.change, file.before.as_deref()), ("new/dir/b.txt", &Change::Added, None));
    assert_eq!(hunks(file), [(0..0, 0..2)]);
}

#[test]
fn a_call_that_only_reads_takes_no_baseline_and_lists_no_file() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let mut tracker = repo.begin();
    repo.announce(&mut tracker, "t1", ToolKind::Read, "a.txt");
    repo.announce(&mut tracker, "t2", ToolKind::Shell, "a.txt");
    assert!(repo.files(tracker).is_empty());
}

#[test]
fn a_file_written_back_the_way_it_was_is_not_a_change() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let mut tracker = repo.begin();
    repo.announce(&mut tracker, "t1", ToolKind::Write, "a.txt");
    repo.write("a.txt", "x\n");
    assert!(repo.files(tracker).is_empty());
}

#[test]
fn a_file_beyond_the_project_is_tracked_by_its_absolute_path() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "z.txt", b"z\n");
    let (z, fresh) = (outside.path().join("z.txt"), outside.path().join("new/n.txt"));
    let mut tracker = repo.begin();
    let start = |id: &str, kind: ToolKind, file: &Path| Event::ToolStarted(ToolCall {
        id: ToolId::new(id),
        name: "Tool".into(),
        kind,
        input: serde_json::Value::Null,
        file: Some(file.to_string_lossy().to_string()),
        parent: None,
        status: ToolStatus::Running,
    });
    tracker.observe(repo.project.as_ref(), &start("t1", ToolKind::Edit, &z));
    tracker.observe(repo.project.as_ref(), &start("t2", ToolKind::Write, &fresh));
    fs::write(&z, "z\nmore\n").unwrap();
    write(outside.path(), "new/n.txt", b"n\n");
    let mut files = repo.files(tracker);
    files.sort_by(|a, b| a.path.cmp(&b.path));
    assert_eq!(files.len(), 2, "{files:?}");
    let by = |path: &Path| files.iter().find(|f| f.path == path.to_string_lossy()).unwrap_or_else(|| panic!("{path:?} is listed"));
    assert_eq!((&by(&z).change, by(&z).before.as_deref(), by(&z).after.as_deref()), (&Change::Modified, Some("z\n"), Some("z\nmore\n")));
    assert_eq!((&by(&fresh).change, by(&fresh).before.as_deref()), (&Change::Added, None));
    assert_eq!(by(&z).counts(), (1, 0));
}

/// A project opened through a symlink, with the agent naming the real path (as macOS does for /tmp): the file is the project's own,
/// kept by its relative path, not a second file beyond the project.
#[cfg(unix)]
#[test]
fn a_file_named_by_its_real_path_in_a_project_opened_through_a_symlink_is_the_projects_own() {
    let repo = Repo::with(&[("sub/a.txt", "one\n")]);
    let link = tempfile::tempdir().unwrap();
    let through = link.path().join("through");
    std::os::unix::fs::symlink(repo.root(), &through).unwrap();
    let project = Arc::new(LocalProject::open(through.join("sub")).unwrap());
    let real = fs::canonicalize(repo.root()).unwrap().join("sub/a.txt");
    let mut tracker = TurnTracker::begin(project.as_ref());
    tracker.observe(project.as_ref(), &Event::ToolStarted(ToolCall {
        id: ToolId::new("t1"),
        name: "Edit".into(),
        kind: ToolKind::Edit,
        input: serde_json::Value::Null,
        file: Some(real.display().to_string()),
        parent: None,
        status: ToolStatus::Running,
    }));
    fs::write(&real, "one\ntwo\n").unwrap();
    let files = tracker.finish(project.as_ref()).files().to_vec();
    assert_eq!(files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["a.txt"], "one file, by its relative path: {files:?}");
}

#[test]
fn a_path_that_climbs_or_names_the_folder_is_left_alone() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let mut tracker = repo.begin();
    let root = repo.root().display().to_string();
    for file in [format!("{root}/../etc/passwd"), format!("{root}/./a.txt"), root.clone(), format!("{root}/"), "/tmp/../etc/hosts".to_string(), "relative.txt".to_string(), "/".to_string()] {
        tracker.observe(repo.project.as_ref(), &Event::ToolStarted(ToolCall {
            id: ToolId::new("t"),
            name: "Write".into(),
            kind: ToolKind::Write,
            input: serde_json::Value::Null,
            file: Some(file),
            parent: None,
            status: ToolStatus::Running,
        }));
    }
    assert!(repo.files(tracker).is_empty());
}

#[test]
fn a_file_a_shell_command_changed_is_found_by_git_with_the_last_commits_text_as_its_baseline() {
    let repo = Repo::with(&[("a.txt", "1\n2\n"), ("b.txt", "keep\n")]);
    let tracker = repo.begin();
    repo.write("a.txt", "1\n2\n3\n");
    let files = repo.files(tracker);
    assert_eq!(files.len(), 1);
    let file = &files[0];
    assert_eq!((file.path.as_str(), file.exact, file.before.as_deref()), ("a.txt", true, Some("1\n2\n")));
    assert_eq!(hunks(file), [(2..2, 2..3)]);
}

#[test]
fn a_file_a_shell_command_created_is_added() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let tracker = repo.begin();
    repo.write("gen/out.txt", "made by a script\n");
    let file = &repo.files(tracker)[0];
    assert_eq!((file.path.as_str(), &file.change, file.before.as_deref(), file.exact), ("gen/out.txt", &Change::Added, None, true));
}

#[test]
fn a_file_a_shell_command_deleted_is_deleted_with_its_text() {
    let repo = Repo::with(&[("a.txt", "x\ny\n"), ("b.txt", "z\n")]);
    let tracker = repo.begin();
    fs::remove_file(repo.root().join("a.txt")).unwrap();
    let file = &repo.files(tracker)[0];
    assert_eq!((file.path.as_str(), &file.change, file.after.as_deref()), ("a.txt", &Change::Deleted, None));
    assert_eq!(hunks(file), [(0..2, 2..2)]);
}

#[test]
fn a_file_moved_by_a_command_is_renamed_not_deleted_and_added() {
    let repo = Repo::with(&[("old.txt", "same text\n"), ("other.txt", "o\n")]);
    let tracker = repo.begin();
    fs::rename(repo.root().join("old.txt"), repo.root().join("new.txt")).unwrap();
    let files = repo.files(tracker);
    assert_eq!(files.len(), 1, "{files:#?}");
    assert_eq!((files[0].path.as_str(), &files[0].change), ("new.txt", &Change::Renamed { from: "old.txt".into() }));
    assert!(hunks(&files[0]).is_empty());
}

#[test]
fn a_moved_and_edited_file_shows_as_a_delete_and_an_add() {
    let repo = Repo::with(&[("old.txt", "same text\n")]);
    let tracker = repo.begin();
    fs::remove_file(repo.root().join("old.txt")).unwrap();
    repo.write("new.txt", "same text, edited\n");
    let changes: Vec<_> = repo.files(tracker).iter().map(|f| (f.path.clone(), f.change.clone())).collect();
    assert_eq!(changes, [("new.txt".into(), Change::Added), ("old.txt".into(), Change::Deleted)]);
}

#[test]
fn a_file_the_user_had_changed_and_a_command_changed_again_has_an_approximate_baseline() {
    let repo = Repo::with(&[("a.txt", "committed\n")]);
    repo.write("a.txt", "user edit\n");
    let tracker = repo.begin();
    repo.write("a.txt", "user edit\nscript line\n");
    let file = &repo.files(tracker)[0];
    assert!(!file.exact, "the user's text before the command is not recoverable");
    assert_eq!(file.before.as_deref(), Some("committed\n"));
}

#[test]
fn a_changed_file_the_user_had_not_yet_committed_but_no_command_touched_is_not_listed() {
    let repo = Repo::with(&[("a.txt", "committed\n")]);
    repo.write("a.txt", "user edit\n");
    repo.write("untracked.txt", "u\n");
    let tracker = repo.begin();
    assert!(repo.files(tracker).is_empty());
}

#[test]
fn an_untracked_file_a_command_changed_is_listed_without_a_baseline() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    repo.write("scratch.txt", "one\n");
    let tracker = repo.begin();
    repo.write("scratch.txt", "one\ntwo\n");
    let file = &repo.files(tracker)[0];
    assert_eq!(file.content, Content::Unknown);
    assert_eq!(file.after.as_deref(), Some("one\ntwo\n"));
}

#[test]
fn a_change_the_user_had_made_and_a_command_undid_is_listed_as_unknown() {
    let repo = Repo::with(&[("a.txt", "committed\n")]);
    repo.write("a.txt", "user edit\n");
    let tracker = repo.begin();
    git(repo.root(), &["checkout", "--", "a.txt"]);
    let file = &repo.files(tracker)[0];
    assert_eq!((file.path.as_str(), &file.content), ("a.txt", &Content::Unknown));
}

#[test]
fn a_file_a_tool_and_git_both_name_is_listed_once_with_the_tools_baseline() {
    let repo = Repo::with(&[("a.txt", "committed\n")]);
    let mut tracker = repo.begin();
    repo.announce(&mut tracker, "t1", ToolKind::Edit, "a.txt");
    repo.write("a.txt", "edited\n");
    let files = repo.files(tracker);
    assert_eq!(files.len(), 1);
    assert!(files[0].exact);
}

#[test]
fn a_binary_file_is_listed_with_no_hunks_and_an_unchanged_one_is_not() {
    let repo = Repo::with(&[("logo.bin", "\0\u{1}\u{2}"), ("still.bin", "\0same")]);
    let mut tracker = repo.begin();
    repo.announce(&mut tracker, "t1", ToolKind::Write, "logo.bin");
    repo.announce(&mut tracker, "t2", ToolKind::Write, "still.bin");
    write(repo.root(), "logo.bin", b"\0\x09\x09\x09");
    let files = repo.files(tracker);
    assert_eq!(files.len(), 1);
    assert_eq!((files[0].path.as_str(), &files[0].content, &files[0].change), ("logo.bin", &Content::Binary, &Change::Modified));
    assert_eq!(files[0].counts(), (0, 0));
}

#[test]
fn a_file_of_bytes_that_are_not_utf8_is_binary() {
    let repo = Repo::with(&[("a.dat", "text\n")]);
    let mut tracker = repo.begin();
    repo.announce(&mut tracker, "t1", ToolKind::Write, "a.dat");
    write(repo.root(), "a.dat", &[0xff, 0xfe, 0x41]);
    assert_eq!(repo.files(tracker)[0].content, Content::Binary);
}

#[test]
fn a_project_that_is_not_a_repository_has_only_the_files_tools_named() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.txt", b"old\n");
    write(dir.path(), "b.txt", b"b\n");
    let project = Arc::new(LocalProject::open(dir.path()).unwrap());
    let mut tracker = TurnTracker::begin(project.as_ref());
    let call = ToolCall {
        id: ToolId::new("t"),
        name: "Edit".into(),
        kind: ToolKind::Edit,
        input: serde_json::Value::Null,
        file: Some(dir.path().join("a.txt").to_string_lossy().to_string()),
        parent: None,
        status: ToolStatus::Running,
    };
    tracker.observe(project.as_ref(), &Event::ToolStarted(call));
    write(dir.path(), "a.txt", b"new\n");
    write(dir.path(), "b.txt", b"changed by a command\n");
    let files = tracker.finish(project.as_ref());
    let paths: Vec<_> = files.files().iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["a.txt"], "no git, so a command's change goes unseen");
}

#[test]
fn a_project_in_a_folder_of_a_larger_repository_sees_only_its_own_files() {
    let repo = Repo::with(&[("web/a.txt", "1\n"), ("api/b.txt", "1\n")]);
    let project = Arc::new(LocalProject::open(repo.root().join("web")).unwrap());
    let tracker = TurnTracker::begin(project.as_ref());
    repo.write("web/a.txt", "2\n");
    repo.write("api/b.txt", "2\n");
    let turn = tracker.finish(project.as_ref());
    let paths: Vec<_> = turn.files().iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["a.txt"]);
    assert_eq!(turn.files()[0].before.as_deref(), Some("1\n"), "the last commit's text, found from the project's folder");
}

#[test]
fn a_later_turn_diffs_against_the_text_the_user_kept() {
    let repo = Repo::with(&[("a.txt", "1\n2\n3\n")]);
    // Turn one: the agent changes rows 1 and 3.
    let mut first = repo.begin();
    repo.announce(&mut first, "t1", ToolKind::Edit, "a.txt");
    repo.write("a.txt", "ONE\n2\nTHREE\n");
    let one = repo.files(first);
    assert_eq!(one[0].counts(), (2, 2));
    // The user keeps the first change and rejects the second: the file on disk says so.
    repo.write("a.txt", "ONE\n2\n3\n");
    git(repo.root(), &["commit", "-q", "-am", "kept"]);
    // Turn two starts from that text.
    let mut second = repo.begin();
    repo.announce(&mut second, "t2", ToolKind::Edit, "a.txt");
    repo.write("a.txt", "ONE\nTWO\n3\n");
    let two = repo.files(second);
    assert_eq!(two[0].before.as_deref(), Some("ONE\n2\n3\n"));
    assert_eq!(hunks(&two[0]), [(1..2, 2..3)]);
}

#[test]
fn the_files_come_sorted_by_path() {
    let repo = Repo::with(&[("b.txt", "1\n"), ("a.txt", "1\n"), ("c/d.txt", "1\n")]);
    let tracker = repo.begin();
    for path in ["c/d.txt", "b.txt", "a.txt"] {
        repo.write(path, "2\n");
    }
    let paths: Vec<_> = repo.files(tracker).iter().map(|f| f.path.clone()).collect();
    assert_eq!(paths, ["a.txt", "b.txt", "c/d.txt"]);
}

/// The events of a captured `claude` run, with the run's folder replaced by this repository's.
fn captured(name: &str, root: &Path) -> Vec<Event> {
    captured_replacing(name, root, "", "")
}

/// A captured run, with the text `from` in it swapped for `to` (an empty `from` swaps nothing).
fn captured_replacing(name: &str, root: &Path, from: &str, to: &str) -> Vec<Event> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agents/tests/fixtures/claude_code").join(format!("{name}.jsonl"));
    let mut text = fs::read_to_string(path).unwrap();
    if !from.is_empty() {
        assert!(text.contains(from), "the run holds {from:?}");
        text = text.replace(from, to);
    }
    let mut mapper = ClaudeLineMapper::new();
    let now = std::time::Instant::now();
    let root = root.to_string_lossy().to_string();
    text.lines()
        .flat_map(|line| mapper.line(&line.replace("/tmp/cc/work", &root).replace("\"/work/", &format!("\"{root}/")), now))
        .collect()
}

#[test]
fn a_captured_edit_run_takes_its_baseline_before_the_edit_and_finds_the_hunk() {
    let repo = Repo::with(&[("edit_me.txt", "hello world\nsecond line\nthird line\n")]);
    let mut tracker = repo.begin();
    let mut edited = false;
    for event in captured("edit_file", repo.root()) {
        tracker.observe(repo.project.as_ref(), &event);
        // The edit lands when the tool runs: after the call is announced with its file.
        if !edited && matches!(event, Event::ToolInput { .. }) && tracker_has_edit(&event) {
            repo.write("edit_me.txt", "goodbye world\nsecond line\nthird line\n");
            edited = true;
        }
    }
    assert!(edited, "the run has an Edit call");
    let file = &repo.files(tracker)[0];
    assert_eq!(file.before.as_deref(), Some("hello world\nsecond line\nthird line\n"));
    assert_eq!(hunks(file), [(0..1, 1..2)]);
}

fn tracker_has_edit(event: &Event) -> bool {
    matches!(event, Event::ToolInput { input, .. } if input.get("old_string").is_some())
}

#[test]
fn a_captured_write_run_lists_the_new_file_as_added() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let mut tracker = repo.begin();
    for event in captured("permission_allow", repo.root()) {
        tracker.observe(repo.project.as_ref(), &event);
        if matches!(event, Event::Permission(_)) {
            // The user allowed it and the tool wrote the file.
            repo.write("made.txt", "hi");
        }
    }
    let file = &repo.files(tracker)[0];
    assert_eq!((file.path.as_str(), &file.change, file.after.as_deref()), ("made.txt", &Change::Added, Some("hi")));
    assert_eq!(hunks(file), [(0..0, 0..1)]);
}

#[test]
fn a_captured_run_that_writes_nothing_lists_nothing() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let mut tracker = repo.begin();
    for event in captured("permission_deny", repo.root()) {
        tracker.observe(repo.project.as_ref(), &event);
    }
    assert!(repo.files(tracker).is_empty(), "the write was denied, so the file never appeared");
}

/// A shell tool call, the way a stream gives it: the call starts with no input, then its whole input arrives.
fn shell_call(tracker: &mut TurnTracker, project: &dyn atelier_project::Project, id: &str, command: &str) {
    tracker.observe(project, &Event::ToolStarted(ToolCall {
        id: ToolId::new(id),
        name: "Bash".into(),
        kind: ToolKind::Shell,
        input: serde_json::Value::Null,
        file: None,
        parent: None,
        status: ToolStatus::Running,
    }));
    tracker.observe(project, &Event::ToolInput { id: ToolId::new(id), input: serde_json::json!({ "command": command }), file: None });
}

fn outside_folder() -> (TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = |name: &str| dir.path().join(name).to_string_lossy().to_string();
    let first = path("out.txt");
    (dir, first)
}

#[test]
fn a_file_a_shell_redirect_makes_beyond_the_project_is_listed_as_added() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let (outside, out) = outside_folder();
    let mut tracker = repo.begin();
    shell_call(&mut tracker, repo.project.as_ref(), "t1", &format!("echo hello > {out}"));
    fs::write(&out, "hello\n").unwrap();
    let files = repo.files(tracker);
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!((files[0].path.as_str(), &files[0].change, files[0].before.as_deref(), files[0].after.as_deref()), (out.as_str(), &Change::Added, None, Some("hello\n")));
    assert!(files[0].exact);
    assert_eq!(files[0].counts(), (1, 0), "one line added");
    drop(outside);
}

#[test]
fn a_file_a_shell_redirect_overwrites_beyond_the_project_keeps_the_text_it_had() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let (_outside, out) = outside_folder();
    fs::write(&out, "old\n").unwrap();
    let mut tracker = repo.begin();
    shell_call(&mut tracker, repo.project.as_ref(), "t1", &format!("echo new > {out}"));
    fs::write(&out, "new\n").unwrap();
    let files = repo.files(tracker);
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!((&files[0].change, files[0].before.as_deref(), files[0].after.as_deref()), (&Change::Modified, Some("old\n"), Some("new\n")));
}

#[test]
fn tee_and_an_append_make_files_beyond_the_project_too() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let (outside, first) = outside_folder();
    let second = outside.path().join("log.txt").to_string_lossy().to_string();
    fs::write(&second, "one\n").unwrap();
    let mut tracker = repo.begin();
    shell_call(&mut tracker, repo.project.as_ref(), "t1", &format!("echo a | tee {first} && echo two >> {second}"));
    fs::write(&first, "a\n").unwrap();
    fs::write(&second, "one\ntwo\n").unwrap();
    let mut files = repo.files(tracker);
    files.sort_by(|a, b| a.path.cmp(&b.path));
    assert_eq!(files.len(), 2, "{files:?}");
    let by = |path: &str| files.iter().find(|f| f.path == path).unwrap();
    assert_eq!((&by(&first).change, by(&first).before.as_deref()), (&Change::Added, None));
    assert_eq!((&by(&second).change, by(&second).before.as_deref()), (&Change::Modified, Some("one\n")));
}

#[test]
fn a_shell_command_seen_after_it_ran_still_lists_the_file_it_wrote() {
    // The baseline is read when the call's input is whole, and the command may already have run by then (a slow link).
    // The file must not vanish because its text before is no longer known.
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let (_outside, out) = outside_folder();
    let mut tracker = repo.begin();
    fs::write(&out, "hello\n").unwrap();
    shell_call(&mut tracker, repo.project.as_ref(), "t1", &format!("echo hello > {out}"));
    let files = repo.files(tracker);
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!((files[0].path.as_str(), files[0].after.as_deref(), files[0].before.as_deref(), files[0].exact), (out.as_str(), Some("hello\n"), None, false));
}

#[test]
fn a_shell_command_that_failed_to_write_lists_nothing() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let (_outside, out) = outside_folder();
    let mut tracker = repo.begin();
    shell_call(&mut tracker, repo.project.as_ref(), "t1", &format!("false > {out}"));
    assert!(repo.files(tracker).is_empty());
}

#[test]
fn a_shell_command_that_writes_to_a_device_or_a_stream_lists_nothing() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let mut tracker = repo.begin();
    shell_call(&mut tracker, repo.project.as_ref(), "t1", "make > /dev/null 2>&1; echo oops >&2");
    assert!(repo.files(tracker).is_empty());
}

#[test]
fn a_shell_write_inside_the_project_is_listed_once_by_its_relative_path() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let mut tracker = repo.begin();
    shell_call(&mut tracker, repo.project.as_ref(), "t1", &format!("echo hello > {}", repo.abs("new.txt")));
    repo.write("new.txt", "hello\n");
    let files = repo.files(tracker);
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!((files[0].path.as_str(), &files[0].change, files[0].exact), ("new.txt", &Change::Added, true));
}

#[test]
fn a_shell_command_waiting_for_a_permission_has_its_file_read_before_the_user_answers() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let (_outside, out) = outside_folder();
    fs::write(&out, "old\n").unwrap();
    let mut tracker = repo.begin();
    let call = ToolCall {
        id: ToolId::new("t1"),
        name: "Bash".into(),
        kind: ToolKind::Shell,
        input: serde_json::json!({ "command": format!("echo new > {out}") }),
        file: None,
        parent: None,
        status: ToolStatus::Pending,
    };
    let ask = PermissionRequest { id: atelier_agents::session::RequestId::new("r"), call, reason: None, choices: vec![] };
    tracker.observe(repo.project.as_ref(), &Event::Permission(ask));
    fs::write(&out, "new\n").unwrap();
    let files = repo.files(tracker);
    assert_eq!((files[0].before.as_deref(), files[0].after.as_deref()), (Some("old\n"), Some("new\n")));
}

fn errored(id: &str) -> Event {
    Event::ToolFinished { id: ToolId::new(id), output: ToolOutput { text: "denied".into(), is_error: true, truncated: false, full_at: None } }
}

#[test]
fn a_shell_command_that_failed_leaves_an_unchanged_file_out_of_the_list() {
    // Denied by the user, or failed before it wrote: the file is as it was, and listing it would say the agent touched it.
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let (_outside, out) = outside_folder();
    fs::write(&out, "old\n").unwrap();
    let mut tracker = repo.begin();
    shell_call(&mut tracker, repo.project.as_ref(), "t1", &format!("echo new > {out}"));
    tracker.observe(repo.project.as_ref(), &errored("t1"));
    assert!(repo.files(tracker).is_empty());
}

#[test]
fn a_shell_command_that_failed_after_it_wrote_still_lists_the_file() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let (_outside, out) = outside_folder();
    let mut tracker = repo.begin();
    shell_call(&mut tracker, repo.project.as_ref(), "t1", &format!("make > {out}"));
    fs::write(&out, "partial log\n").unwrap();
    tracker.observe(repo.project.as_ref(), &errored("t1"));
    let files = repo.files(tracker);
    assert_eq!((files.len(), &files[0].change), (1, &Change::Added));
}

#[test]
fn a_captured_shell_run_takes_the_text_of_a_redirect_target_before_the_command_would_run() {
    // A real Claude Code run of one Bash call, its command swapped for a redirect beyond the project. The command reaches
    // the tracker in pieces, then whole; the file is written only once it is whole, as the tool would run it.
    let repo = Repo::with(&[("note.txt", "n\n")]);
    let (_outside, out) = outside_folder();
    let mut tracker = repo.begin();
    let mut written = false;
    for event in captured_replacing("tool_read", repo.root(), "cat note.txt", &format!("echo hello > {out}")) {
        tracker.observe(repo.project.as_ref(), &event);
        let whole = match &event {
            Event::ToolInput { input, .. } => input.get("command"),
            Event::ToolStarted(call) => call.input.get("command"),
            _ => None,
        };
        if !written && whole.and_then(|command| command.as_str()).is_some_and(|command| command.ends_with(&out)) {
            fs::write(&out, "hello\n").unwrap();
            written = true;
        }
    }
    assert!(written, "the whole command reached the tracker");
    let files = repo.files(tracker);
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!((files[0].path.as_str(), &files[0].change, files[0].before.as_deref(), files[0].exact), (out.as_str(), &Change::Added, None, true));
}
