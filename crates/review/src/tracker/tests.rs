//! The tracker on real git repositories and real files, through the project, and on a captured run.
use std::{fs, path::Path, process::Command, sync::Arc};

use lathe_agents::{
    claude_code::Mapper,
    session::{Event, PermissionRequest, ToolCall, ToolId, ToolKind, ToolStatus},
};
use lathe_project::LocalProject;
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
    let ask = PermissionRequest { id: lathe_agents::session::RequestId::new("r"), call, reason: None, choices: vec![] };
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
fn a_file_outside_the_project_is_left_alone() {
    let repo = Repo::with(&[("a.txt", "x\n")]);
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "z.txt", b"z");
    let mut tracker = repo.begin();
    let event = Event::ToolStarted(ToolCall {
        id: ToolId::new("t1"),
        name: "Write".into(),
        kind: ToolKind::Write,
        input: serde_json::Value::Null,
        file: Some(outside.path().join("z.txt").to_string_lossy().to_string()),
        parent: None,
        status: ToolStatus::Running,
    });
    tracker.observe(repo.project.as_ref(), &event);
    let dotdot = Event::ToolTarget { id: ToolId::new("t1"), file: format!("{}/../etc/passwd", repo.root().display()) };
    tracker.observe(repo.project.as_ref(), &dotdot);
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
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agents/tests/fixtures/claude_code").join(format!("{name}.jsonl"));
    let text = fs::read_to_string(path).unwrap();
    let mut mapper = Mapper::new();
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
