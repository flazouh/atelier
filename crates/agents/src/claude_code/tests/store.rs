use serde_json::json;

use crate::{
    claude_code::store::{parse_listing, slug},
    session::SessionId,
};

fn user(text: &str) -> String {
    json!({"type": "user", "message": {"role": "user", "content": text}}).to_string()
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
