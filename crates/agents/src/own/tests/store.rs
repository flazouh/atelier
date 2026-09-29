use serde_json::json;

use super::support::rig;
use crate::{
    own::{
        message::{Block, Message, Role},
        store::{self, Meta, events_of, title_of, valid_id},
    },
    session::{Event, PermissionMode, SessionError, SessionId, ToolKind},
};

fn project() -> (tempfile::TempDir, std::sync::Arc<dyn lathe_project::Project>) {
    let dir = tempfile::tempdir().unwrap();
    let project: std::sync::Arc<dyn lathe_project::Project> = std::sync::Arc::new(crate::testing::Locked(std::sync::Arc::new(lathe_project::LocalProject::open(dir.path()).unwrap())));
    (dir, project)
}

fn meta(id: &str, title: &str, updated: u64) -> Meta {
    Meta { id: id.into(), title: title.into(), model: "m".into(), updated }
}

#[test]
fn a_record_saves_and_loads_with_every_kind_of_block() {
    let (_dir, project) = project();
    store::ensure_dir(project.as_ref()).unwrap();
    let messages = vec![
        Message::user("hello"),
        Message::assistant(vec![
            Block::Thinking { text: "hm".into(), signature: Some("sig".into()) },
            Block::Redacted { data: "R".into() },
            Block::Text { text: "text with \"quotes\"\nand a newline".into() },
            Block::ToolUse { id: "t".into(), name: "read".into(), input: json!({"path": "a"}) },
        ]),
        Message { role: Role::User, blocks: vec![Block::ToolResult { id: "t".into(), content: "out".into(), is_error: true }] },
    ];
    store::save(project.as_ref(), &meta("own-1", "hello", 5), &messages).unwrap();
    let (loaded_meta, loaded) = store::load(project.as_ref(), "own-1").unwrap();
    assert_eq!(loaded, messages);
    assert_eq!(loaded_meta, meta("own-1", "hello", 5));
}

#[test]
fn an_id_is_never_a_path() {
    for good in ["own-19a3f-0", "abc123", "A-b"] {
        assert!(valid_id(good), "{good}");
    }
    for bad in ["", "../x", "a/b", "a b", "a.b", "é", &"x".repeat(81)] {
        assert!(!valid_id(bad), "{bad:?}");
    }
    let (_dir, project) = project();
    assert!(matches!(store::load(project.as_ref(), "../secret"), Err(SessionError::Read(_))));
    assert!(matches!(store::load(project.as_ref(), "missing"), Err(SessionError::Read(_))));
}

#[test]
fn a_damaged_record_names_the_line() {
    let (dir, project) = project();
    store::ensure_dir(project.as_ref()).unwrap();
    store::save(project.as_ref(), &meta("bad", "t", 1), &[Message::user("a")]).unwrap();
    std::fs::write(dir.path().join(".lathe/agent/sessions/bad.jsonl"), "{\"role\":\"user\",\"blocks\":[]}\nnot json\n").unwrap();
    let error = store::load(project.as_ref(), "bad").unwrap_err();
    assert!(error.to_string().contains("line 2"), "{error}");
}

#[test]
fn the_list_is_newest_first_and_skips_records_with_no_title() {
    let (_dir, project) = project();
    store::ensure_dir(project.as_ref()).unwrap();
    store::save(project.as_ref(), &meta("old", "older one", 1), &[]).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    store::save(project.as_ref(), &meta("new", "newer one", 2), &[]).unwrap();
    store::save(project.as_ref(), &meta("blank", "", 3), &[]).unwrap();
    let list = store::list(project.as_ref()).unwrap();
    let titles: Vec<_> = list.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["newer one", "older one"]);
    assert_eq!(list[0].id, SessionId::new("new"));
    assert!(store::list(&*self::project().1).unwrap().is_empty(), "no folder, no sessions");
}

#[test]
fn a_title_is_the_first_line_cut_short() {
    assert_eq!(title_of("  \n\nFix the bug\nin detail"), "Fix the bug");
    assert_eq!(title_of(&"a".repeat(300)).chars().count(), 101);
    assert!(title_of(&"a".repeat(300)).ends_with('…'));
    assert_eq!(title_of(""), "");
}

#[test]
fn history_shows_what_was_said_and_each_call_with_its_result() {
    let messages = vec![
        Message::user("fix it"),
        Message::assistant(vec![
            Block::Thinking { text: "plan".into(), signature: None },
            Block::Text { text: "Looking.".into() },
            Block::ToolUse { id: "t1".into(), name: "edit".into(), input: json!({"path": "a.rs", "old_string": "a", "new_string": "b"}) },
        ]),
        Message { role: Role::User, blocks: vec![Block::ToolResult { id: "t1".into(), content: "Edited a.rs".into(), is_error: false }] },
        Message::assistant(vec![Block::Text { text: "Done.".into() }]),
    ];
    let events = events_of(&messages);
    assert!(matches!(&events[0], Event::UserMessage { text } if text == "fix it"));
    assert!(matches!(&events[1], Event::Thinking { delta, .. } if delta == "plan"));
    assert!(matches!(&events[2], Event::ThinkingDone { .. }));
    let call = events.iter().find_map(|e| if let Event::ToolStarted(c) = e { Some(c.clone()) } else { None }).unwrap();
    assert_eq!((call.kind, call.file.as_deref()), (ToolKind::Edit, Some("a.rs")));
    assert!(events.iter().any(|e| matches!(e, Event::ToolFinished { id, output } if id.as_str() == "t1" && output.text == "Edited a.rs")));
    let mut conversation = crate::session::Conversation::new();
    events.iter().for_each(|e| conversation.apply(e));
    assert!(!conversation.working());
}

#[test]
fn a_session_writes_its_record_into_the_project_where_the_project_can_see_it() {
    let rig = rig(vec![super::server::Step::Sse(super::server::says("hi"))], PermissionMode::Ask);
    rig.send("record me");
    rig.turns(1);
    let id = rig.session_id();
    let base = rig.dir.path().join(".lathe/agent/sessions");
    let lines = std::fs::read_to_string(base.join(format!("{}.jsonl", id.as_str()))).unwrap();
    assert_eq!(lines.lines().count(), 2, "the question and the answer, one a line");
    let meta: Meta = serde_json::from_slice(&std::fs::read(base.join(format!("{}.meta", id.as_str()))).unwrap()).unwrap();
    assert_eq!((meta.title.as_str(), meta.model.as_str()), ("record me", "claude-opus-5-5"));
    assert!(meta.updated > 0);
    assert!(!lines.contains(super::support::KEY) && !std::fs::read_to_string(base.join(format!("{}.meta", id.as_str()))).unwrap().contains(super::support::KEY));
}

#[test]
fn a_session_that_cannot_save_says_so_once_and_keeps_working() {
    let rig = rig(vec![super::server::Step::Sse(super::server::says("one")), super::server::Step::Sse(super::server::says("two"))], PermissionMode::Ask);
    // A file where the folder should be makes every save fail.
    std::fs::create_dir_all(rig.dir.path().join(".lathe")).unwrap();
    std::fs::write(rig.dir.path().join(".lathe/agent"), "in the way").unwrap();
    rig.send("a");
    rig.turns(1);
    rig.send("b");
    let events = rig.turns(2);
    assert_eq!(events.iter().filter(|e| matches!(e, Event::Warning(w) if w.contains("not being saved"))).count(), 1);
    assert_eq!(super::support::turn_ends(&events).len(), 2);
}

fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn repo() -> super::support::Rig {
    let rig = rig(vec![super::server::Step::Sse(super::server::says("one")), super::server::Step::Sse(super::server::says("two"))], PermissionMode::Ask);
    git(rig.dir.path(), &["init", "-q"]);
    rig
}

/// The record lives in the project. It must not show up as something to commit.
#[test]
fn a_session_in_a_git_repository_leaves_git_status_clean() {
    let _guard = crate::testing::spawn_lock();
    let rig = repo();
    drop(_guard);
    rig.send("hello");
    rig.turns(1);
    assert!(rig.dir.path().join(".lathe/agent/sessions").is_dir(), "the record was written");
    assert_eq!(git(rig.dir.path(), &["status", "--porcelain"]), "", "and git does not see it");
    let exclude = std::fs::read_to_string(rig.dir.path().join(".git/info/exclude")).unwrap();
    assert_eq!(exclude.lines().filter(|l| *l == ".lathe/").count(), 1);
    assert!(git(rig.dir.path(), &["ls-files", "--others", "--exclude-standard"]).is_empty());
}

#[test]
fn the_line_is_added_once_and_what_was_there_stays() {
    let _guard = crate::testing::spawn_lock();
    let rig = repo();
    drop(_guard);
    // A file with no final newline, as an editor may leave it.
    std::fs::write(rig.dir.path().join(".git/info/exclude"), "# mine\n*.log").unwrap();
    rig.send("a");
    rig.turns(1);
    rig.send("b");
    rig.turns(2);
    let exclude = std::fs::read_to_string(rig.dir.path().join(".git/info/exclude")).unwrap();
    assert_eq!(exclude, "# mine\n*.log\n.lathe/\n");
    assert!(!store::exclude_from_git(rig.project.as_ref()).unwrap(), "asked again, it finds the line");
    assert_eq!(git(rig.dir.path(), &["status", "--porcelain"]), "");
}

#[test]
fn a_project_that_is_not_a_git_repository_gets_no_git_folder_and_no_error() {
    let rig = rig(vec![super::server::Step::Sse(super::server::says("one"))], PermissionMode::Ask);
    rig.send("hi");
    let events = rig.turns(1);
    assert!(!rig.dir.path().join(".git").exists());
    assert!(rig.dir.path().join(".lathe/agent/sessions").is_dir());
    assert!(!events.iter().any(|e| matches!(e, Event::Warning(_))));
    assert_eq!(store::exclude_from_git(rig.project.as_ref()), Ok(false));
}

#[test]
fn a_linked_worktree_writes_the_shared_exclude_file() {
    let _guard = crate::testing::spawn_lock();
    let main = tempfile::tempdir().unwrap();
    git(main.path(), &["init", "-q"]);
    git(main.path(), &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "first"]);
    // A linked worktree, laid out by hand: a machine may forbid `git worktree add`, and this is all it is.
    // The worktree has a `.git` file that points at a folder in the main repository's `.git/worktrees`.
    let tree = tempfile::tempdir().unwrap();
    let path = tree.path().join("wt");
    let admin = main.path().join(".git/worktrees/wt");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::create_dir_all(&admin).unwrap();
    std::fs::write(path.join(".git"), format!("gitdir: {}\n", admin.display())).unwrap();
    std::fs::write(admin.join("HEAD"), std::fs::read(main.path().join(".git/HEAD")).unwrap()).unwrap();
    std::fs::write(admin.join("commondir"), "../..\n").unwrap();
    std::fs::write(admin.join("gitdir"), format!("{}\n", path.join(".git").display())).unwrap();
    assert!(git(&path, &["rev-parse", "--git-path", "info/exclude"]).trim().ends_with("info/exclude"));
    let project: std::sync::Arc<dyn lathe_project::Project> = std::sync::Arc::new(lathe_project::LocalProject::open(&path).unwrap());
    assert_eq!(store::exclude_from_git(project.as_ref()), Ok(true));
    assert_eq!(std::fs::read_to_string(main.path().join(".git/info/exclude")).unwrap().lines().filter(|l| *l == ".lathe/").count(), 1);
}
