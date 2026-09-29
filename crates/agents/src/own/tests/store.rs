use serde_json::json;

use super::support::rig;
use crate::{
    own::{
        message::{Block, Message, Role},
        store::{self, Meta, events_of, title_of, valid_id},
    },
    session::{Event, PermissionMode, SessionError, SessionId, ToolKind},
};

/// A project on a temporary folder, with its data folder in another.
fn project() -> (tempfile::TempDir, tempfile::TempDir, std::sync::Arc<dyn lathe_project::Project>) {
    let dir = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let local = lathe_project::LocalProject::open(dir.path()).unwrap().with_data_dir(data.path());
    let project: std::sync::Arc<dyn lathe_project::Project> = std::sync::Arc::new(crate::testing::Locked(std::sync::Arc::new(local)));
    (dir, data, project)
}

fn meta(id: &str, title: &str, updated: u64) -> Meta {
    Meta { id: id.into(), title: title.into(), model: "m".into(), updated }
}

#[test]
fn a_record_saves_and_loads_with_every_kind_of_block() {
    let (_dir, _data, project) = project();
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
    let (_dir, _data, project) = project();
    assert!(matches!(store::load(project.as_ref(), "../secret"), Err(SessionError::Read(_))));
    assert!(matches!(store::load(project.as_ref(), "missing"), Err(SessionError::Read(_))));
}

#[test]
fn a_damaged_record_names_the_line() {
    let (_dir, _data, project) = project();
    store::save(project.as_ref(), &meta("bad", "t", 1), &[Message::user("a")]).unwrap();
    project.data_write("agent/sessions/bad.jsonl", "{\"role\":\"user\",\"blocks\":[]}\nnot json\n".as_bytes()).unwrap();
    let error = store::load(project.as_ref(), "bad").unwrap_err();
    assert!(error.to_string().contains("line 2"), "{error}");
}

#[test]
fn the_list_is_newest_first_and_skips_records_with_no_title() {
    let (_dir, _data, project) = project();
    store::save(project.as_ref(), &meta("old", "older one", 1), &[]).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    store::save(project.as_ref(), &meta("new", "newer one", 2), &[]).unwrap();
    store::save(project.as_ref(), &meta("blank", "", 3), &[]).unwrap();
    let list = store::list(project.as_ref()).unwrap();
    let titles: Vec<_> = list.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["newer one", "older one"]);
    assert_eq!(list[0].id, SessionId::new("new"));
    assert!(store::list(&*self::project().2).unwrap().is_empty(), "no folder, no sessions");
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
fn a_session_writes_its_record_into_the_data_folder_and_nothing_into_the_project() {
    let rig = rig(vec![super::server::Step::Sse(super::server::says("hi"))], PermissionMode::Ask);
    rig.send("record me");
    rig.turns(1);
    let id = rig.session_id();
    let lines = String::from_utf8(rig.project.data_read(&format!("agent/sessions/{}.jsonl", id.as_str())).unwrap()).unwrap();
    assert_eq!(lines.lines().count(), 2, "the question and the answer, one a line");
    let meta: Meta = serde_json::from_slice(&rig.project.data_read(&format!("agent/sessions/{}.meta", id.as_str())).unwrap()).unwrap();
    assert_eq!((meta.title.as_str(), meta.model.as_str()), ("record me", "claude-opus-5-5"));
    assert!(meta.updated > 0);
    assert!(!lines.contains(super::support::KEY));
    assert_eq!(std::fs::read_dir(rig.dir.path()).unwrap().count(), 0, "the project folder is as it was: no .lathe");
}

#[test]
fn a_session_that_cannot_save_says_so_once_and_keeps_working() {
    let rig = rig(vec![super::server::Step::Sse(super::server::says("one")), super::server::Step::Sse(super::server::says("two"))], PermissionMode::Ask);
    // A file where the data folder's `projects` folder should be makes every save fail.
    std::fs::write(rig.data.path().join("projects"), "in the way").unwrap();
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

/// The record lives outside the repository: it is nothing to commit, and lathe touches no git file for it.
#[test]
fn a_session_in_a_git_repository_leaves_git_status_clean_and_git_alone() {
    let _guard = crate::testing::spawn_lock();
    let rig = rig(vec![super::server::Step::Sse(super::server::says("one"))], PermissionMode::Ask);
    git(rig.dir.path(), &["init", "-q"]);
    drop(_guard);
    let before = std::fs::read(rig.dir.path().join(".git/info/exclude")).unwrap_or_default();
    rig.send("hello");
    rig.turns(1);
    assert!(!rig.project.data_list("agent/sessions").unwrap().is_empty(), "the record was written");
    assert_eq!(git(rig.dir.path(), &["status", "--porcelain"]), "", "and git does not see it");
    assert_eq!(std::fs::read(rig.dir.path().join(".git/info/exclude")).unwrap_or_default(), before, "the exclude file is untouched");
    assert!(!rig.dir.path().join(".lathe").exists());
}

#[test]
fn a_project_that_is_not_a_git_repository_gets_no_git_folder_and_no_error() {
    let rig = rig(vec![super::server::Step::Sse(super::server::says("one"))], PermissionMode::Ask);
    rig.send("hi");
    let events = rig.turns(1);
    assert!(!rig.dir.path().join(".git").exists());
    assert!(!events.iter().any(|e| matches!(e, Event::Warning(_))));
}

/// What an older lathe left in the project.
fn legacy(dir: &std::path::Path, id: &str, title: &str, updated: u64, text: &str) {
    let base = dir.join(".lathe/agent/sessions");
    std::fs::create_dir_all(&base).unwrap();
    let line = serde_json::to_string(&Message::user(text)).unwrap();
    std::fs::write(base.join(format!("{id}.jsonl")), format!("{line}\n")).unwrap();
    std::fs::write(base.join(format!("{id}.meta")), serde_json::to_vec(&meta(id, title, updated)).unwrap()).unwrap();
}

#[test]
fn an_old_record_in_the_project_moves_to_the_data_folder_when_the_list_is_read() {
    let (dir, _data, project) = project();
    legacy(dir.path(), "own-old", "the old one", 10, "hello");
    legacy(dir.path(), "own-older", "the older one", 5, "hi");
    let list = store::list(project.as_ref()).unwrap();
    let titles: Vec<_> = list.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["the old one", "the older one"], "the order is the time in the meta, not the time of the move");
    assert!(project.data_read("agent/sessions/own-old.jsonl").is_ok() && project.data_read("agent/sessions/own-old.meta").is_ok());
    assert!(!dir.path().join(".lathe").exists(), "what was moved is gone from the project, and so are its empty folders");
    let (loaded, messages) = store::load(project.as_ref(), "own-old").unwrap();
    assert_eq!((loaded.title.as_str(), messages.len()), ("the old one", 1));
    assert_eq!(store::migrate(project.as_ref()), Ok(0), "asked again there is nothing to move");
}

#[test]
fn an_old_session_asked_for_by_id_is_moved_and_loaded() {
    let (dir, _data, project) = project();
    legacy(dir.path(), "own-x", "by id", 3, "hey");
    let (loaded, _) = store::load(project.as_ref(), "own-x").unwrap();
    assert_eq!(loaded.title, "by id");
    assert!(project.data_read("agent/sessions/own-x.meta").is_ok());
}

#[test]
fn a_session_that_is_in_both_places_keeps_the_data_folders_copy() {
    let (dir, _data, project) = project();
    store::save(project.as_ref(), &meta("own-both", "newer, in the data folder", 9), &[Message::user("new")]).unwrap();
    legacy(dir.path(), "own-both", "older, in the project", 1, "old");
    store::migrate(project.as_ref()).unwrap();
    let (loaded, messages) = store::load(project.as_ref(), "own-both").unwrap();
    assert_eq!(loaded.title, "newer, in the data folder");
    assert_eq!(messages, vec![Message::user("new")]);
    assert!(!dir.path().join(".lathe").exists());
}

#[test]
fn a_half_written_old_record_is_left_where_it_is() {
    let (dir, _data, project) = project();
    legacy(dir.path(), "own-half", "half", 1, "x");
    std::fs::remove_file(dir.path().join(".lathe/agent/sessions/own-half.jsonl")).unwrap();
    assert_eq!(store::migrate(project.as_ref()), Ok(0), "no messages, no session moved");
    assert!(store::list(project.as_ref()).unwrap().is_empty());
    assert!(dir.path().join(".lathe/agent/sessions/own-half.meta").exists(), "and nothing of it was removed");
}
