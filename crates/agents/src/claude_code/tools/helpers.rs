use serde_json::Value;

use crate::partial_json::string_end;

/// The tool that asks the reader a question and takes the answer back in its input.
pub const ASK_QUESTION: &str = "AskUserQuestion";

use crate::session::{FileEdit, ToolKind};
use super::types::{FILE_KEYS, TodoTool};

pub(in super::super) fn kind(name: &str) -> ToolKind {
    match name {
        "Read" => ToolKind::Read,
        "Edit" | "MultiEdit" | "NotebookEdit" => ToolKind::Edit,
        "Write" => ToolKind::Write,
        "Grep" | "Glob" => ToolKind::Search,
        "Bash" | "BashOutput" | "KillShell" => ToolKind::Shell,
        "WebFetch" | "WebSearch" => ToolKind::Fetch,
        _ => ToolKind::Other,
    }
}

/// Whether a tool's input is text the reader can watch being written: an edit's old and new text, a file's content.
pub(in super::super) fn streams_input(name: &str) -> bool {
    matches!(name, "Edit" | "Write")
}

/// The text an Edit or a Write changes, in the words every agent shares; a text not there yet is empty. `None` for
/// another tool, and until the file is named. A MultiEdit has no one text and shows as its row.
pub(in super::super) fn edit_of(name: &str, input: &Value) -> Option<FileEdit> {
    let text = |key: &str| input.get(key).and_then(Value::as_str).unwrap_or("").to_string();
    let path = input.get("file_path")?.as_str()?.to_string();
    match name {
        "Edit" => Some(FileEdit { path, old: text("old_string"), new: text("new_string") }),
        "Write" => Some(FileEdit { path, old: String::new(), new: text("content") }),
        _ => None,
    }
}

/// The file a call names, from the argument the tool uses for it.
pub(in super::super) fn file(input: &Value) -> Option<String> {
    FILE_KEYS.iter().find_map(|key| input.get(key)?.as_str()).map(str::to_string)
}

/// The file a call names, read from the start of its input while the JSON still streams in. Only a key
/// of the outermost object counts, so the same words inside a string (a file's new text) do not, and
/// only a value whose closing quote has arrived.
pub(in super::super) fn file_in_partial_input(json: &str) -> Option<String> {
    let bytes = json.as_bytes();
    let (mut depth, mut i) = (0usize, 0usize);
    while i < bytes.len() {
        match bytes[i] {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth = depth.saturating_sub(1),
            b'"' => {
                let end = string_end(bytes, i)?;
                if depth == 1 {
                    let key: String = serde_json::from_str(&json[i..=end]).ok()?;
                    let after = json[end + 1..].trim_start();
                    if FILE_KEYS.contains(&key.as_str()) && after.starts_with(':') {
                        let value = after[1..].trim_start();
                        if value.starts_with('"') {
                            let stop = string_end(value.as_bytes(), 0)?;
                            return serde_json::from_str(&value[..=stop]).ok();
                        }
                        return None;
                    }
                }
                i = end;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// The tools that start a subagent.
pub(in super::super) fn starts_subagent(name: &str) -> bool {
    matches!(name, "Agent" | "Task")
}

pub(in super::super) fn todo_tool(name: &str) -> Option<TodoTool> {
    match name {
        "TodoWrite" => Some(TodoTool::Write),
        "TaskCreate" => Some(TodoTool::Create),
        "TaskUpdate" => Some(TodoTool::Update),
        _ => None,
    }
}
