use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use super::types::{Content, ControlBody, StreamEvent};

/// A `system` line: `init`, `task_started`, `task_progress`, `task_notification`, and more that
/// atelier ignores. One shape holds every field any of them uses.
#[derive(Deserialize)]
pub(in super::super) struct System {
    pub subtype: String,
    pub session_id: Option<String>,
    /// The init's own commands, by name without the slash.
    #[serde(default)]
    pub slash_commands: Vec<String>,
    pub model: Option<String>,
    #[serde(rename = "permissionMode")]
    pub permission_mode: Option<String>,
    pub tool_use_id: Option<String>,
    pub description: Option<String>,
    pub subagent_type: Option<String>,
    /// `local_agent` for a subagent, `local_bash` for a shell command run in the background.
    pub task_type: Option<String>,
    pub status: Option<String>,
    pub summary: Option<String>,
}

#[derive(Deserialize)]
pub(in super::super) struct Stream {
    pub event: StreamEvent,
    pub parent_tool_use_id: Option<String>,
}

#[derive(Deserialize)]
pub(in super::super) struct Started {
    pub id: String,
}

#[derive(Deserialize)]
pub(in super::super) struct Body {
    pub id: Option<String>,
    pub content: Content,
    /// An assistant message's own tokens: what its request carried, so how full the context is.
    #[serde(default)]
    pub usage: Option<RawUsage>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Deserialize)]
pub(in super::super) struct Message {
    pub message: Body,
    pub parent_tool_use_id: Option<String>,
    #[serde(alias = "toolUseResult")]
    pub tool_use_result: Option<Value>,
    /// A transcript marks a subagent's own lines and lines atelier should not show.
    #[serde(default, rename = "isSidechain")]
    pub sidechain: bool,
    #[serde(default, rename = "isMeta")]
    pub meta: bool,
}

#[derive(Deserialize)]
pub(in super::super) struct Finish {
    pub subtype: String,
    #[serde(default)]
    pub is_error: bool,
    pub result: Option<String>,
    pub terminal_reason: Option<String>,
    pub total_cost_usd: Option<f64>,
    pub usage: Option<RawUsage>,
    #[serde(default)]
    pub errors: Vec<String>,
    /// Each model the turn used, by name.
    #[serde(default, rename = "modelUsage")]
    pub model_usage: HashMap<String, ModelUsage>,
}

#[derive(Deserialize)]
pub(in super::super) struct ModelUsage {
    #[serde(rename = "contextWindow")]
    pub context_window: Option<u64>,
}

#[derive(Deserialize, Default)]
pub(in super::super) struct RawUsage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
}

#[derive(Deserialize)]
pub(in super::super) struct ControlRequest {
    pub request_id: String,
    pub request: ControlBody,
}

#[derive(Deserialize)]
pub(in super::super) struct CanUseTool {
    pub tool_name: String,
    #[serde(default)]
    pub input: Value,
    pub tool_use_id: Option<String>,
    pub description: Option<String>,
    pub permission_suggestions: Option<Value>,
}
