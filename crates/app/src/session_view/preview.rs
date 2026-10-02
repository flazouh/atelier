//! What an approval shows in place of a tool's raw input: an Edit or a MultiEdit as a diff, a Write as
//! all-new lines, a Bash call as its command, each under the path relative to the project. The raw input
//! stays behind "View details".
use atelier_ui::tool_preview::{TextEdit, ToolPreview, relative_path};
use atelier_agents::session::ToolCall;
use serde_json::Value;

/// The preview of `call`, whose paths are shown relative to `root`; `None` for a call with none, or
/// whose input is not known yet.
pub fn preview(call: &ToolCall, root: &str) -> Option<ToolPreview> {
    let input = &call.input;
    let text = |value: &Value, key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
    let path = || text(input, "file_path").map(|p| relative_path(&p, root));
    match call.name.as_str() {
        "Edit" => Some(ToolPreview::edit(path()?, text(input, "old_string")?, text(input, "new_string")?)),
        "MultiEdit" => {
            let edits = input.get("edits")?.as_array()?;
            let edits: Option<Vec<TextEdit>> = edits.iter().map(|e| Some(TextEdit::new(text(e, "old_string")?, text(e, "new_string")?))).collect();
            Some(ToolPreview::edits(path()?, edits?))
        }
        "Write" => Some(ToolPreview::written(path()?, text(input, "content")?)),
        "Bash" => Some(ToolPreview::command(text(input, "command")?)),
        _ => None,
    }
}

/// The diff of an edit or a write as far as its input has come: a text not yet there is empty, so old text alone reads as
/// removed lines and the new text joins as it arrives. `None` until the file is known, and for a call that is no edit.
pub fn streamed(call: &ToolCall, root: &str) -> Option<ToolPreview> {
    let input = &call.input;
    let text = |key: &str| input.get(key).and_then(Value::as_str).unwrap_or("").to_string();
    let path = relative_path(input.get("file_path")?.as_str()?, root);
    match call.name.as_str() {
        "Edit" => Some(ToolPreview::edit(path, text("old_string"), text("new_string"))),
        "Write" => Some(ToolPreview::written(path, text("content"))),
        "MultiEdit" => preview(call, root),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
