use serde_json::Value;

use crate::{
    session::{TodoStatus, ToolOutput},
};

/// Whether a `task_started` names a subagent. A task with no type is an older `claude`'s subagent.
pub(super) fn is_agent_task(task_type: Option<&str>) -> bool {
    task_type.is_none_or(|t| t.contains("agent"))
}

pub(super) fn todo_status(status: Option<&str>) -> TodoStatus {
    match status {
        Some("in_progress") => TodoStatus::InProgress,
        Some("completed") => TodoStatus::Done,
        _ => TodoStatus::Pending,
    }
}

/// `Task #3 created successfully` gives `3`.
pub(super) fn task_number(text: &str) -> Option<String> {
    let digits: String = text.split("#").nth(1)?.chars().take_while(char::is_ascii_digit).collect();
    (!digits.is_empty()).then_some(digits)
}

/// A tool result's text: a string, or the text blocks of a list.
pub(super) fn flatten(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| block.get("text")?.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// `claude` keeps an output it finds too large in a file and puts a preview and the path in the
/// result. atelier passes both on. An output that is large and not kept is cut to its head.
pub(super) fn tool_output(text: &str, is_error: bool) -> ToolOutput {
    const OPEN: &str = "<persisted-output>";
    const SAVED: &str = "Full output saved to: ";
    const PREVIEW: &str = "Preview";
    if let Some(body) = text.strip_prefix(OPEN) {
        let full_at = body.split_once(SAVED).and_then(|(_, rest)| rest.lines().next()).map(str::to_string);
        let preview = body
            .split_once(PREVIEW)
            .and_then(|(_, rest)| rest.split_once('\n'))
            .map_or("", |(_, preview)| preview.trim_end().trim_end_matches("</persisted-output>").trim_end_matches("...").trim_end());
        return ToolOutput { truncated: true, full_at, ..ToolOutput::head(preview, is_error) };
    }
    ToolOutput::head(text, is_error)
}
