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
    ["file_path", "notebook_path", "path"].iter().find_map(|key| input.get(key)?.as_str()).map(str::to_string)
}

/// The tools that start a subagent.
pub(super) fn starts_subagent(name: &str) -> bool {
    matches!(name, "Agent" | "Task")
}

/// The tools that write the todo list. lathe shows the list, not the calls.
pub(super) fn edits_todos(name: &str) -> bool {
    matches!(name, "TodoWrite" | "TaskCreate" | "TaskUpdate")
}
