use serde::de::DeserializeOwned;
use serde_json::Value;

use super::super::wire;
use crate::session::{Event, SessionId, SessionSummary, TurnOutcome, Usage};
use super::structs::Step;
use super::types::{LISTED, TITLE_MAX};

/// atelier's name for a model: its id without the settings Cursor puts in brackets after it.
pub(super) fn model_name(id: &str) -> String {
    id.split_once('[').map_or(id, |(name, _)| name).to_string()
}

pub(super) fn warning(text: String) -> Step {
    Step { events: vec![Event::Warning(text)], ..Step::default() }
}

pub(super) fn parse<T: DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// Why a turn stopped, in atelier's words. A turn the model could not finish is a failed one that says why.
pub(super) fn stop_outcome(reason: &str) -> TurnOutcome {
    match reason {
        "end_turn" => TurnOutcome::Completed,
        "cancelled" => TurnOutcome::Interrupted,
        "max_tokens" => TurnOutcome::Failed("The reply reached the model's token limit.".into()),
        "max_turn_requests" => TurnOutcome::Failed("The turn reached the agent's limit of model requests.".into()),
        "refusal" => TurnOutcome::Failed("The model refused to go on.".into()),
        other => TurnOutcome::Failed(format!("The turn stopped: {other}.")),
    }
}

pub(super) fn usage(usage: wire::PromptUsage) -> Usage {
    Usage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_read_tokens: usage.cached_read_tokens.unwrap_or(0),
        cache_write_tokens: usage.cached_write_tokens.unwrap_or(0),
        cost_usd: None,
    }
}

/// The agent's sessions as rows, newest first.
pub(super) fn summaries(listed: wire::Listed) -> Vec<SessionSummary> {
    let mut rows: Vec<SessionSummary> = listed
        .sessions
        .into_iter()
        .map(|s| SessionSummary {
            id: SessionId::new(s.session_id),
            title: s.title.map(|t| t.trim().chars().take(TITLE_MAX).collect()).filter(|t: &String| !t.is_empty()).unwrap_or_else(|| "Untitled session".into()),
            updated: s.updated_at.as_deref().and_then(super::super::time::epoch_seconds),
            account: None,
        })
        .collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.updated));
    rows.truncate(LISTED);
    rows
}
