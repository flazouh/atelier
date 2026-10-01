use lathe_agents::session::{Call, ToolCall, ToolId, ToolKind, ToolStatus};
use serde_json::json;

use super::*;

fn call(name: &str, kind: ToolKind, input: serde_json::Value, file: Option<&str>, status: ToolStatus) -> Call {
    Call { call: ToolCall { id: ToolId::new("t"), name: name.into(), kind, input, file: file.map(Into::into), parent: None, status }, output: None }
}

fn plain(path: &str) -> String {
    path.trim_start_matches("/work/").to_string()
}

/// A shell call says what it is for and what it ran, not "Bash".
#[test]
fn a_shell_call_says_what_it_is_for_and_what_it_ran() {
    let c = call("Bash", ToolKind::Shell, json!({ "command": "cargo test -p beui\nmore", "description": "Run the beui tests" }), None, ToolStatus::Done);
    let s = summary(&c, plain);
    assert_eq!((s.title.as_str(), s.detail.as_deref()), ("Run the beui tests", Some("cargo test -p beui")));
}

/// Without a description, the title is the verb and the command is the argument; while it runs the verb is "Running".
#[test]
fn a_shell_call_with_only_a_command_shows_the_command() {
    let done = summary(&call("Bash", ToolKind::Shell, json!({ "command": "ls -la" }), None, ToolStatus::Done), plain);
    assert_eq!((done.title.as_str(), done.detail.as_deref()), ("Ran", Some("ls -la")));
    let running = summary(&call("Bash", ToolKind::Shell, json!({ "command": "ls -la" }), None, ToolStatus::Running), plain);
    assert_eq!(running.title, "Running");
}

#[test]
fn a_read_names_its_file_relative_to_the_project() {
    let s = summary(&call("Read", ToolKind::Read, json!({ "file_path": "/work/AGENTS.md" }), Some("/work/AGENTS.md"), ToolStatus::Done), plain);
    assert_eq!((s.title.as_str(), s.file.as_deref(), s.detail), ("Read", Some("AGENTS.md"), None));
}

#[test]
fn a_search_shows_its_pattern_and_where() {
    let s = summary(&call("Grep", ToolKind::Search, json!({ "pattern": "fn main", "path": "/work/src" }), None, ToolStatus::Done), plain);
    assert_eq!((s.title.as_str(), s.detail.as_deref()), ("Searched", Some("fn main in src")));
    let g = summary(&call("Glob", ToolKind::Search, json!({ "pattern": "**/*.rs" }), None, ToolStatus::Done), plain);
    assert_eq!((g.title.as_str(), g.detail.as_deref()), ("Found files", Some("**/*.rs")));
}

#[test]
fn the_web_calls_show_their_address_and_query() {
    let f = summary(&call("WebFetch", ToolKind::Fetch, json!({ "url": "https://example.com" }), None, ToolStatus::Done), plain);
    assert_eq!((f.title.as_str(), f.detail.as_deref()), ("Fetched", Some("https://example.com")));
    let w = summary(&call("WebSearch", ToolKind::Other, json!({ "query": "gpui table" }), None, ToolStatus::Running), plain);
    assert_eq!((w.title.as_str(), w.detail.as_deref()), ("Searching the web", Some("gpui table")));
}

/// An unknown tool keeps its own name as the title and shows the first argument it can.
#[test]
fn an_unknown_tool_keeps_its_name() {
    let s = summary(&call("mcp__x__lookup", ToolKind::Other, json!({ "query": "abc" }), None, ToolStatus::Done), plain);
    assert_eq!((s.title.as_str(), s.detail.as_deref()), ("mcp__x__lookup", Some("abc")));
}

/// A long command is cut to one line of at most 160 characters.
#[test]
fn a_long_argument_is_cut_to_one_line() {
    let long = "x".repeat(300);
    let s = summary(&call("Bash", ToolKind::Shell, json!({ "command": long }), None, ToolStatus::Done), plain);
    assert_eq!(s.detail.unwrap().chars().count(), 160);
}
