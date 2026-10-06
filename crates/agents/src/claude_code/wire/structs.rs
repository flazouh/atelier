use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use super::types::{Content, ControlBody, StreamEvent};

/// The `error` of the assistant line `claude` writes when it has no sign-in, and of the `api_retry` it writes when the
/// sign-in it has is refused.
const SIGN_IN_FAILED: &str = "authentication_failed";
/// The name of an error in a line. `claude` writes it as a word, such as `rate_limit`, or as a map with a `type`; a line
/// with anything else is still read, with no name, rather than refused whole.
fn error_name<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Ok(match Option::<Value>::deserialize(deserializer)? {
        Some(Value::String(name)) => Some(name),
        Some(Value::Object(map)) => map.get("type").and_then(Value::as_str).map(str::to_string),
        _ => None,
    })
}

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
    /// An `api_retry`'s reason: `authentication_failed`, `overloaded`, and more. A word, or a map with a `type`.
    #[serde(default, deserialize_with = "error_name")]
    pub error: Option<String>,
    /// An `api_retry`'s HTTP status.
    pub error_status: Option<u16>,
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
    /// The live stream's mark for a line `claude` wrote itself, such as its nudge after an empty reply.
    #[serde(default, rename = "isSynthetic")]
    pub synthetic: bool,
    /// Who wrote a user line, in a transcript: the reader, or `claude` itself, as with a background
    /// task's notice.
    #[serde(default)]
    pub origin: Option<Origin>,
    /// Why `claude` wrote this assistant line itself instead of the model: `authentication_failed` for a run with no
    /// sign-in, `rate_limit` for a limit, and more.
    #[serde(default, deserialize_with = "error_name")]
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub(in super::super) struct Origin {
    pub kind: String,
}

impl System {
    /// Whether this is a retry of a request the API refused for want of a sign-in. `claude` retries it ten times over
    /// minutes before it says so in an assistant line, so this is the first sign.
    pub fn is_sign_in_retry(&self) -> bool {
        self.subtype == "api_retry" && (self.error.as_deref() == Some(SIGN_IN_FAILED) || self.error_status == Some(401))
    }
}

impl Message {
    /// Whether this is the line `claude` writes for a run with no sign-in.
    pub fn is_signed_out(&self) -> bool {
        self.error.as_deref() == Some(SIGN_IN_FAILED)
    }

    /// Whether `claude` wrote this user line itself, rather than the reader.
    pub fn written_by_claude(&self) -> bool {
        self.meta || self.synthetic || self.origin.as_ref().is_some_and(|origin| origin.kind != "human")
    }
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
    /// The ids of the messages this turn took, from atelier's and any folded in between tool rounds.
    /// An older `claude` leaves it out.
    #[serde(default)]
    pub user_message_uuids: Option<Vec<String>>,
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
pub(in super::super) struct RateLimit {
    #[serde(default)]
    pub rate_limit_info: Option<RateLimitInfo>,
}

/// `status` is `allowed`, `allowed_warning` or `rejected`; `rateLimitType` names the window, such as
/// `five_hour` or `seven_day`.
#[derive(Deserialize)]
pub(in super::super) struct RateLimitInfo {
    pub status: String,
    #[serde(rename = "resetsAt")]
    pub resets_at: Option<u64>,
    #[serde(rename = "rateLimitType")]
    pub rate_limit_type: Option<String>,
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

/// The answer to a request atelier wrote. Only the id and the body are read here; what the body means depends on the
/// request that has the id.
#[derive(Deserialize)]
pub(in super::super) struct ControlResponse {
    pub response: ControlAnswer,
}

#[derive(Deserialize)]
pub(in super::super) struct ControlAnswer {
    #[serde(default)]
    pub request_id: String,
    #[serde(default)]
    pub response: Value,
}

/// What `claude` tells of its context window: each part with its tokens, the total and the window.
#[derive(Deserialize)]
pub(in super::super) struct ContextUsage {
    #[serde(default)]
    pub categories: Vec<ContextCategory>,
    #[serde(rename = "totalTokens")]
    pub total_tokens: u64,
    #[serde(rename = "maxTokens")]
    pub max_tokens: Option<u64>,
    #[serde(rename = "rawMaxTokens")]
    pub raw_max_tokens: Option<u64>,
}

/// `kind` is `used` for what is in the window; the free space, the autocompact buffer and the tools that load on
/// demand have other kinds and are not in it.
#[derive(Deserialize)]
pub(in super::super) struct ContextCategory {
    pub name: String,
    pub tokens: u64,
    #[serde(default)]
    pub kind: String,
}
