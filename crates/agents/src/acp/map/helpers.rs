use serde_json::Value;

use super::super::wire::{self, ToolContent};
use crate::session::{ChoiceKind, Todo, TodoStatus, ToolKind, ToolOutput, ToolStatus};
use super::structs::Seen;

/// A call announced as already ended starts as running; its `ToolFinished` follows at once.
pub(super) fn open_status(status: ToolStatus) -> ToolStatus {
    match status {
        ToolStatus::Done | ToolStatus::Failed => ToolStatus::Running,
        open => open,
    }
}

/// ACP's tool kinds in atelier's words. A diff of a new file is a write.
pub(super) fn tool_kind(kind: Option<&str>, content: &[ToolContent]) -> ToolKind {
    match kind {
        Some("read") => ToolKind::Read,
        Some("edit") if content.iter().any(|c| matches!(c, ToolContent::Diff(diff) if diff.creates())) => ToolKind::Write,
        Some("edit" | "delete" | "move") => ToolKind::Edit,
        Some("search") => ToolKind::Search,
        Some("execute") => ToolKind::Shell,
        Some("fetch") => ToolKind::Fetch,
        _ => ToolKind::Other,
    }
}

pub(super) fn status(status: Option<&str>) -> Option<ToolStatus> {
    match status? {
        "pending" => Some(ToolStatus::Pending),
        "in_progress" => Some(ToolStatus::Running),
        "completed" => Some(ToolStatus::Done),
        "failed" => Some(ToolStatus::Failed),
        _ => None,
    }
}

pub(super) fn choice_kind(kind: &str) -> ChoiceKind {
    match kind {
        "allow_once" => ChoiceKind::Allow,
        "allow_always" => ChoiceKind::AllowAlways,
        _ => ChoiceKind::Deny,
    }
}

pub(super) fn todo(index: usize, entry: wire::PlanEntry) -> Todo {
    let status = match entry.status.as_str() {
        "in_progress" => TodoStatus::InProgress,
        "completed" => TodoStatus::Done,
        _ => TodoStatus::Pending,
    };
    Todo { id: index.to_string(), text: entry.content, status }
}

/// The file a call names: its first location, else the path of a diff it carries.
pub(super) fn file_of(update: &wire::ToolCall) -> Option<String> {
    let located = update.locations.as_ref().and_then(|l| l.first()).map(|l| l.path.clone());
    located.or_else(|| diff_path(update.content.as_deref().unwrap_or_default()))
}

pub(super) fn diff_path(content: &[ToolContent]) -> Option<String> {
    content.iter().find_map(|c| match c {
        ToolContent::Diff(diff) => Some(diff.path.clone()),
        _ => None,
    })
}

/// What a call returned: the text of its content and a line for each diff, else its raw output.
pub(super) fn output(seen: &Seen, failed: bool) -> ToolOutput {
    let text = content_text(&seen.content);
    let text = if text.is_empty() { seen.raw_output.as_ref().map(raw_text).unwrap_or_default() } else { text };
    ToolOutput::head(&text, failed)
}

pub(super) fn content_text(content: &[ToolContent]) -> String {
    let parts: Vec<String> = content
        .iter()
        .filter_map(|c| match c {
            ToolContent::Content { content } => Some(content.text().to_string()).filter(|t| !t.is_empty()),
            ToolContent::Diff(diff) if diff.creates() => Some(format!("Created {}", diff.path)),
            ToolContent::Diff(diff) => Some(format!("Changed {}", diff.path)),
            ToolContent::Other => None,
        })
        .collect();
    parts.join("\n")
}

/// The text of a raw output, which ACP leaves to the agent: a string, or an object whose `content`,
/// `output`, `stdout` or `text` holds it (Cursor's read and shell results), with a `stderr` after. A
/// command that printed nothing gives empty text. Any other shape shows as its JSON.
fn raw_text(raw: &Value) -> String {
    let field = |key: &str| raw.get(key).and_then(Value::as_str);
    match raw {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        Value::Object(_) => {
            let fields = ["content", "output", "stdout", "text"];
            let text = fields.into_iter().filter_map(field).find(|t| !t.is_empty());
            let named = fields.into_iter().any(|key| field(key).is_some());
            match (text, field("stderr")) {
                (Some(text), Some(stderr)) if !stderr.is_empty() => format!("{}\n{stderr}", text.trim_end_matches('\n')),
                (Some(text), _) => text.to_string(),
                (None, Some(stderr)) => stderr.to_string(),
                (None, None) if named => String::new(),
                (None, None) => raw.to_string(),
            }
        }
        other => other.to_string(),
    }
}
