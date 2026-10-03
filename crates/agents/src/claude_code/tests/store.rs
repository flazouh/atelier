use std::{fs, path::Path, process};

use serde_json::json;
use tempfile::TempDir;

use crate::{
    claude_code::store::{list_script, parse_listing, read_script, slug},
    session::SessionId,
};

const PROJECT_SLUG: &str = "-home-alex-code-atelier";
const OTHER_PROJECT_SLUG: &str = "-home-alex-code-other";
const USUAL_FOLDER: &str = ".claude";
const WORK_FOLDER: &str = ".claude-work";
const WORK_ACCOUNT: &str = "work";
const USUAL_SESSION: &str = "aaaa-1111";
const WORK_SESSION: &str = "bbbb-2222";

fn user(text: &str) -> String {
    json!({"type": "user", "message": {"role": "user", "content": text}}).to_string()
}

/// A home folder with Claude Code's session files in it.
struct Home(TempDir);

impl Home {
    fn new() -> Self {
        Self(tempfile::tempdir().expect("a temporary home"))
    }

    /// Saves a session that starts with `first_words` in `config_folder`, for the project `slug`.
    fn save(&self, config_folder: &str, slug: &str, id: &str, first_words: &str) {
        let folder = self.0.path().join(config_folder).join("projects").join(slug);
        fs::create_dir_all(&folder).expect("the project folder is made");
        fs::write(folder.join(format!("{id}.jsonl")), user(first_words) + "\n").expect("the session is saved");
    }

    /// Runs `script` with this home, and returns what it printed.
    fn run(&self, script: &str) -> String {
        let output = process::Command::new("sh").args(["-c", script]).env("HOME", self.0.path()).output().expect("sh runs");
        String::from_utf8_lossy(&output.stdout).into()
    }

    fn path(&self) -> &Path {
        self.0.path()
    }
}

#[test]
fn the_list_holds_the_sessions_of_every_account_each_named_by_its_account() {
    let home = Home::new();
    home.save(USUAL_FOLDER, PROJECT_SLUG, USUAL_SESSION, "on the usual account");
    home.save(WORK_FOLDER, PROJECT_SLUG, WORK_SESSION, "on the work account");

    let mut sessions = parse_listing(&home.run(&list_script(PROJECT_SLUG)));
    sessions.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

    assert_eq!(sessions.len(), 2);
    assert_eq!((sessions[0].id.as_str(), sessions[0].account.as_deref()), (USUAL_SESSION, None));
    assert_eq!((sessions[1].id.as_str(), sessions[1].account.as_deref()), (WORK_SESSION, Some(WORK_ACCOUNT)));
}

#[test]
fn the_list_leaves_out_other_projects() {
    let home = Home::new();
    home.save(WORK_FOLDER, OTHER_PROJECT_SLUG, WORK_SESSION, "another project");

    assert!(parse_listing(&home.run(&list_script(PROJECT_SLUG))).is_empty());
}

#[test]
fn a_session_is_read_from_whichever_account_holds_it() {
    let home = Home::new();
    home.save(WORK_FOLDER, PROJECT_SLUG, WORK_SESSION, "on the work account");

    let transcript = home.run(&read_script(PROJECT_SLUG, &SessionId::new(WORK_SESSION)));

    assert!(transcript.contains("on the work account"), "read from {}", home.path().display());
}

#[test]
fn a_folder_becomes_claudes_project_name() {
    assert_eq!(slug("/home/alex/code/local/atelier"), "-home-alex-code-local-atelier");
    assert_eq!(slug("/tmp/a b.c"), "-tmp-a-b-c");
}

#[test]
fn the_listing_gives_each_session_its_id_its_time_and_its_first_words() {
    let listing = format!(
        "@@ /h/.claude/projects/p/aaa-1.jsonl 1790000100\n{}\n{}\n@@ /h/.claude/projects/p/bbb-2.jsonl 1790000000\n{}\n",
        json!({"type": "summary"}),
        user("Add a\n  dark   mode"),
        user("second"),
    );
    let sessions = parse_listing(&listing);
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].id, SessionId::new("aaa-1"));
    assert_eq!(sessions[0].title, "Add a dark mode");
    assert_eq!(sessions[0].updated, Some(1790000100));
    assert_eq!(sessions[1].title, "second");
}

#[test]
fn a_command_note_and_a_meta_line_are_not_a_title() {
    let listing = format!(
        "@@ /p/a.jsonl 1\n{}\n{}\n{}\n",
        user("<command-name>/model</command-name>"),
        json!({"type": "user", "isMeta": true, "message": {"content": "hidden"}}),
        user("the real one"),
    );
    assert_eq!(parse_listing(&listing)[0].title, "the real one");
}

#[test]
fn a_session_with_no_user_message_is_left_out_and_a_long_title_is_cut() {
    let long = "x".repeat(300);
    let listing = format!("@@ /p/empty.jsonl 1\n{}\n@@ /p/long.jsonl 2\n{}\n", json!({"type": "summary"}), user(&long));
    let sessions = parse_listing(&listing);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title.chars().count(), 100);
    assert!(sessions[0].title.ends_with('…'));
}

#[test]
fn garbage_lines_and_an_empty_listing_give_no_sessions() {
    assert!(parse_listing("").is_empty());
    assert!(parse_listing("garbage\n{\n").is_empty());
}
