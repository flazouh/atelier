//! Claude Code's tool names, in one place: the only file that says which tool does what.
use serde_json::Value;

use crate::session::ToolKind;

pub(super) fn kind(name: &str) -> ToolKind {
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

/// The file a call names, from the argument the tool uses for it.
pub(super) fn file(input: &Value) -> Option<String> {
    FILE_KEYS.iter().find_map(|key| input.get(key)?.as_str()).map(str::to_string)
}

/// The arguments that name a file.
const FILE_KEYS: [&str; 3] = ["file_path", "notebook_path", "path"];

/// The file a call names, read from the start of its input while the JSON still streams in. Only a key
/// of the outermost object counts, so the same words inside a string (a file's new text) do not, and
/// only a value whose closing quote has arrived.
pub(super) fn file_in_partial_input(json: &str) -> Option<String> {
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

/// The index of the quote that closes the string opening at `start`, or `None` while it is still open.
fn string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 1,
            b'"' => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// The tools that start a subagent.
pub(super) fn starts_subagent(name: &str) -> bool {
    matches!(name, "Agent" | "Task")
}

/// The tools that write the todo list. atelier shows the list, not the calls.
#[derive(Clone, Copy)]
pub(super) enum TodoTool {
    /// `TodoWrite` sends the whole list.
    Write,
    Create,
    Update,
}

pub(super) fn todo_tool(name: &str) -> Option<TodoTool> {
    match name {
        "TodoWrite" => Some(TodoTool::Write),
        "TaskCreate" => Some(TodoTool::Create),
        "TaskUpdate" => Some(TodoTool::Update),
        _ => None,
    }
}
