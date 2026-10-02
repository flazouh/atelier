use serde_json::json;
use atelier_agents::session::{ToolCall, ToolId, ToolKind, ToolStatus};
use atelier_ui::tool_preview::{TextEdit, ToolPreview};
use super::*;
fn call(name: &str, input: serde_json::Value) -> ToolCall {
    ToolCall { id: ToolId::new("t"), name: name.into(), kind: ToolKind::Other, input, file: None, parent: None, status: ToolStatus::Pending }
}
const ROOT: &str = "/home/alex/qa/m3";
/// An Edit is a diff under the path relative to the project.
#[test]
fn an_edit_is_a_diff_under_its_relative_path() {
    let edit = call("Edit", json!({"file_path": "/home/alex/qa/m3/NOTES.md", "old_string": "a", "new_string": "a\nb", "replace_all": false}));
    assert_eq!(preview(&edit, ROOT), Some(ToolPreview::edit("NOTES.md", "a", "a\nb")));
}
/// A MultiEdit is its edits in order.
#[test]
fn a_multi_edit_is_its_edits_in_order() {
    let multi = call("MultiEdit", json!({"file_path": "/home/alex/qa/m3/src/lib.rs", "edits": [
        {"old_string": "one", "new_string": "ONE"},
        {"old_string": "two", "new_string": "TWO"}
    ]}));
    assert_eq!(preview(&multi, ROOT), Some(ToolPreview::edits("src/lib.rs", vec![TextEdit::new("one", "ONE"), TextEdit::new("two", "TWO")])));
}
/// A Write is the whole text, all new; a Bash call is its command.
#[test]
fn a_write_is_all_new_and_a_shell_call_is_its_command() {
    let write = call("Write", json!({"file_path": "/elsewhere/x.txt", "content": "hi\n"}));
    assert_eq!(preview(&write, ROOT), Some(ToolPreview::written("/elsewhere/x.txt", "hi\n")), "a path outside stays whole");
    let bash = call("Bash", json!({"command": "cargo test", "description": "Run the tests"}));
    assert_eq!(preview(&bash, ROOT), Some(ToolPreview::command("cargo test")));
}
/// A call with no preview, or with its input still unknown, has none.
#[test]
fn other_calls_have_no_preview() {
    assert_eq!(preview(&call("Read", json!({"file_path": "/home/alex/qa/m3/a"})), ROOT), None);
    assert_eq!(preview(&call("Edit", serde_json::Value::Null), ROOT), None);
}

/// While an edit streams in, what has come shows at once: old text alone reads as removed lines, and the new text
/// joins as it arrives. A call whose file is not known has nothing to show yet.
#[test]
fn an_edit_that_streams_shows_what_has_come() {
    let file = "/home/alex/qa/m3/NOTES.md";
    let only_old = call("Edit", json!({"file_path": file, "old_string": "a\nb"}));
    assert_eq!(streamed(&only_old, ROOT), Some(ToolPreview::edit("NOTES.md", "a\nb", "")));
    let cut_new = call("Edit", json!({"file_path": file, "old_string": "a", "new_string": "a\nb"}));
    assert_eq!(streamed(&cut_new, ROOT), Some(ToolPreview::edit("NOTES.md", "a", "a\nb")));
    let write = call("Write", json!({"file_path": file, "content": "hi"}));
    assert_eq!(streamed(&write, ROOT), Some(ToolPreview::written("NOTES.md", "hi")));
    let no_text_yet = call("Write", json!({"file_path": file}));
    assert_eq!(streamed(&no_text_yet, ROOT), Some(ToolPreview::written("NOTES.md", "")));
    assert_eq!(streamed(&call("Edit", json!({"old_string": "a"})), ROOT), None, "no file yet");
    assert_eq!(streamed(&call("Edit", serde_json::Value::Null), ROOT), None);
}

/// A whole MultiEdit shows as it does in an approval; a Bash call or a read is not an edit.
#[test]
fn only_edits_and_writes_stream() {
    let multi = call("MultiEdit", json!({"file_path": "/w/a", "edits": [{"old_string": "x", "new_string": "y"}]}));
    assert_eq!(streamed(&multi, "/w"), preview(&multi, "/w"));
    assert_eq!(streamed(&call("Bash", json!({"command": "ls"})), ROOT), None);
    assert_eq!(streamed(&call("Read", json!({"file_path": "/w/a"})), ROOT), None);
}
