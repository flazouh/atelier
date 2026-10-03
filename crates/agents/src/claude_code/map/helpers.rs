use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use serde_json::Value;

use super::super::wire::{ModelUsage, RateLimitInfo, RawUsage};
use crate::session::{Limit, LimitState, LimitWindow, TodoStatus, ToolOutput};

/// The tokens a request carried, which is what the context holds: the new input, what the cache gave
/// and what it took in, and the reply that joins them.
pub(super) fn context_tokens(usage: &RawUsage) -> u64 {
    usage.input_tokens + usage.cache_read_input_tokens + usage.cache_creation_input_tokens + usage.output_tokens
}

/// The window of the session's model, or the largest the turn used when the model is not among them.
/// Every window told is kept for [`known_window`].
pub(super) fn context_window(models: &HashMap<String, ModelUsage>, model: Option<&str>) -> Option<u64> {
    let mut known = windows().lock().unwrap_or_else(|p| p.into_inner());
    known.extend(models.iter().filter_map(|(name, usage)| Some((name.clone(), usage.context_window?))));
    model
        .and_then(|name| models.get(name))
        .and_then(|usage| usage.context_window)
        .or_else(|| models.values().filter_map(|usage| usage.context_window).max())
}

/// What `claude` tells of the account's usage limit, or `None` for a status it does not know.
pub(super) fn limit(info: RateLimitInfo) -> Option<Limit> {
    let state = match info.status.as_str() {
        "allowed" => LimitState::Clear,
        "allowed_warning" => LimitState::Near,
        "rejected" => LimitState::Reached,
        _ => return None,
    };
    let window = info.rate_limit_type.as_deref().and_then(|kind| match kind {
        "five_hour" => Some(LimitWindow::FiveHour),
        "overage" => Some(LimitWindow::Overage),
        weekly if weekly.starts_with("seven_day") => Some(LimitWindow::Weekly),
        _ => None,
    });
    Some(Limit { state, resets_at: info.resets_at, window })
}

/// The window of `model` as some session's result told it. Only a result tells a window, so a session
/// resumed from its transcript knows its window before its first turn ends only through another.
pub(super) fn known_window(model: &str) -> Option<u64> {
    windows().lock().unwrap_or_else(|p| p.into_inner()).get(model).copied()
}

fn windows() -> &'static Mutex<HashMap<String, u64>> {
    static WINDOWS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    WINDOWS.get_or_init(Mutex::default)
}

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
