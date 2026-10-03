use serde::Deserialize;
use serde_json::Value;

use super::types::{ContentBlock, SessionUpdate, ToolContent};
use crate::session::FileEdit;

/// The answer to `initialize`.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Initialized {
    #[serde(default)]
    pub agent_capabilities: AgentCapabilities,
    #[serde(default)]
    pub auth_methods: Vec<AuthMethod>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct AgentCapabilities {
    #[serde(default)]
    pub load_session: bool,
    #[serde(default)]
    pub session_capabilities: SessionCapabilities,
}

#[derive(Default, Deserialize)]
pub(in super::super) struct SessionCapabilities {
    /// Present when the agent answers `session/list`.
    pub list: Option<Value>,
}

#[derive(Deserialize)]
pub(in super::super) struct AuthMethod {
    pub id: String,
    /// `agent` (the default when absent) for a method the agent runs when asked to `authenticate`; `terminal` for one the
    /// client runs itself, which `authenticate` must never name.
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
}

impl AuthMethod {
    /// Whether `authenticate` can name it.
    pub fn agent_runs_it(&self) -> bool {
        self.kind.as_deref().is_none_or(|kind| kind == "agent")
    }
}

/// The answer to `session/new` and `session/load`. `load` gives no id: it is the one asked for.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Opened {
    pub session_id: Option<String>,
    pub modes: Option<Modes>,
    /// The models the agent offers, from ACP's unstable model selection.
    pub models: Option<Models>,
    #[serde(default)]
    pub config_options: Vec<ConfigOption>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Modes {
    pub current_mode_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Models {
    pub current_model_id: String,
    #[serde(default)]
    pub available_models: Vec<ModelInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ModelInfo {
    pub model_id: String,
}

/// A setting of the session the agent lets the client change, such as its model.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ConfigOption {
    pub id: String,
    pub category: Option<String>,
    pub current_value: Option<Value>,
}

impl ConfigOption {
    pub fn is(&self, category: &str) -> bool {
        self.category.as_deref() == Some(category)
    }
}

/// A `session/update` notification.
#[derive(Deserialize)]
pub(in super::super) struct Notification {
    pub update: SessionUpdate,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Chunk {
    pub content: ContentBlock,
    /// The message the chunk belongs to, when the agent says.
    pub message_id: Option<String>,
}

/// A `tool_call` or a `tool_call_update`. An update carries only what changed, so every field but the id
/// may be missing.
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ToolCall {
    pub tool_call_id: String,
    pub title: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub content: Option<Vec<ToolContent>>,
    pub locations: Option<Vec<Location>>,
    pub raw_input: Option<Value>,
    pub raw_output: Option<Value>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Diff {
    pub path: String,
    pub old_text: Option<String>,
    #[serde(default)]
    pub new_text: String,
}

impl Diff {
    /// Whether the diff makes a new file: ACP gives it no old text, and Cursor gives `-- /dev/null`.
    pub fn creates(&self) -> bool {
        self.old_text.as_deref().is_none_or(|old| old == "-- /dev/null")
    }

    /// The diff in the words every agent shares; a new file has no old text.
    pub fn edit(&self) -> FileEdit {
        let old = if self.creates() { String::new() } else { self.old_text.clone().unwrap_or_default() };
        FileEdit { path: self.path.clone(), old, new: self.new_text.clone() }
    }
}

#[derive(Clone, Deserialize)]
pub(in super::super) struct Location {
    pub path: String,
}

#[derive(Deserialize)]
pub(in super::super) struct Plan {
    pub entries: Vec<PlanEntry>,
}

#[derive(Deserialize)]
pub(in super::super) struct PlanEntry {
    pub content: String,
    pub status: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Commands {
    pub available_commands: Vec<AvailableCommand>,
}

#[derive(Deserialize)]
pub(in super::super) struct AvailableCommand {
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ConfigOptions {
    pub config_options: Vec<ConfigOption>,
}

/// The agent's request `session/request_permission`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct PermissionAsked {
    pub tool_call: ToolCall,
    pub options: Vec<PermissionOption>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct PermissionOption {
    pub option_id: String,
    pub name: String,
    pub kind: String,
}

/// The answer to `session/prompt`: why the turn stopped, and its tokens when the agent counts them.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Prompted {
    pub stop_reason: String,
    pub usage: Option<PromptUsage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct PromptUsage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    pub cached_read_tokens: Option<u64>,
    pub cached_write_tokens: Option<u64>,
}

/// The answer to `session/list`.
#[derive(Deserialize)]
pub(in super::super) struct Listed {
    pub sessions: Vec<ListedSession>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ListedSession {
    pub session_id: String,
    pub title: Option<String>,
    pub updated_at: Option<String>,
}
