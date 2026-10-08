use std::{fs, path::Path, process};

use serde_json::json;
use tempfile::TempDir;

use crate::{
    claude_code::store::{holder_account, holder_script, list_script, parse_listing, parse_listing_in, read_script, slug},
    session::SessionId,
};

const PROJECT_SLUG: &str = "-home-user-code-atelier";
const OTHER_PROJECT_SLUG: &str = "-home-user-code-other";
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
    assert_eq!(slug("/home/user/code/local/atelier"), "-home-user-code-local-atelier");
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

#[test]
fn the_account_holding_a_session_is_told_by_the_folder_it_is_in() {
    let home = Home::new();
    home.save(USUAL_FOLDER, PROJECT_SLUG, USUAL_SESSION, "on the usual account");
    home.save(WORK_FOLDER, PROJECT_SLUG, WORK_SESSION, "on the work account");

    let holder = |id: &str| holder_account(&home.run(&holder_script(PROJECT_SLUG, &SessionId::new(id))));

    assert_eq!(holder(WORK_SESSION).as_deref(), Some(WORK_ACCOUNT));
    assert_eq!(holder(USUAL_SESSION), None, "the usual account has no name");
    assert_eq!(holder("cccc-3333"), None, "a session no account holds is left to the usual one");
}

/// `claude` names a project's folder by turning every character that is not a letter or a digit into "-", so
/// `/work/my-app` and `/work/my_app` share one. A listing for one of them leaves out the sessions the transcripts
/// say were run in the other.
#[test]
fn a_folder_that_shares_claudes_name_for_another_does_not_list_its_sessions() {
    let line = |cwd: &str, text: &str| json!({"type": "user", "cwd": cwd, "message": {"role": "user", "content": text}}).to_string();
    let listing = format!(
        "@@ /h/.claude/projects/p/aaa-1.jsonl 20\n{}\n@@ /h/.claude/projects/p/bbb-2.jsonl 10\n{}\n@@ /h/.claude/projects/p/ccc-3.jsonl 5\n{}\n",
        line("/work/my-app", "mine"),
        line("/work/my_app", "the other folder's"),
        user("no folder recorded"),
    );
    let titles: Vec<String> = parse_listing_in(&listing, "/work/my-app").into_iter().map(|s| s.title).collect();
    assert_eq!(titles, ["mine", "no folder recorded"]);
    assert_eq!(parse_listing(&listing).len(), 3, "the plain reading still lists every file");
}

/// macOS gives a folder two names, `/tmp/x` and `/private/tmp/x`: the transcripts and the project may use either.
#[test]
fn the_private_prefix_does_not_make_a_folder_another() {
    let line = |cwd: &str| json!({"type": "user", "cwd": cwd, "message": {"role": "user", "content": "x"}}).to_string();
    let listing = format!("@@ /h/p/a.jsonl 1\n{}\n@@ /h/p/b.jsonl 2\n{}\n", line("/private/tmp/qa/proj"), line("/tmp/qa/proj/"));
    assert_eq!(parse_listing_in(&listing, "/tmp/qa/proj").len(), 2);
    assert_eq!(parse_listing_in(&listing, "/private/tmp/qa/proj").len(), 2);
}

/// A transcript that stops after a request, or after a call with no result, says it stopped; one that ends on an answer,
/// or on the reader's own stop, does not.
#[test]
fn a_transcript_that_ends_mid_turn_says_so() {
    let user = r#"{"type":"user","message":{"role":"user","content":"go"}}"#;
    let answer = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"done"}]}}"#;
    let call = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t","name":"Bash","input":{}}]}}"#;
    let stop = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"[Request interrupted by user]"}]}}"#;
    let said = |lines: &[&str]| crate::claude_code::store::history(&lines.join("\n")).iter().any(|e| matches!(e, crate::session::Event::Warning(w) if w.starts_with("This stopped before")));
    assert!(said(&[user]), "a request nobody answered");
    assert!(said(&[user, call]), "a call with no result");
    assert!(!said(&[user, answer]), "an answer ends it");
    assert!(!said(&[user, call, stop]), "the reader's own stop is recorded already");
}
