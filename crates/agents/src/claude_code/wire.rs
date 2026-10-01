//! The lines `claude` writes, as far as atelier reads them. A field atelier does not need is not here, and
//! a line or a block of a kind atelier does not know reads as `Ignored` or `Other`, so a newer `claude`
//! never breaks an older atelier.
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum Line {
    System(System),
    StreamEvent(Stream),
    Assistant(Message),
    User(Message),
    #[serde(rename = "result")]
    Finished(Finish),
    ControlRequest(ControlRequest),
    ControlCancelRequest {
        request_id: String,
    },
    #[serde(other)]
    Ignored,
}

/// A `system` line: `init`, `task_started`, `task_progress`, `task_notification`, and more that
/// atelier ignores. One shape holds every field any of them uses.
#[derive(Deserialize)]
pub(super) struct System {
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
pub(super) struct Stream {
    pub event: StreamEvent,
    pub parent_tool_use_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum StreamEvent {
    MessageStart { message: Started },
    ContentBlockStart { index: u32, content_block: Block },
    ContentBlockDelta { index: u32, delta: Delta },
    ContentBlockStop { index: u32 },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct Started {
    pub id: String,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum Delta {
    #[serde(rename = "text_delta")]
    Text {
        text: String,
    },
    #[serde(rename = "input_json_delta")]
    InputJson {
        #[serde(default)]
        partial_json: String,
    },
    #[serde(rename = "thinking_delta")]
    Thinking {
        #[serde(default)]
        thinking: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum Block {
    Text {
        text: String,
    },
    Thinking {
        #[serde(default)]
        thinking: String,
    },
    ToolUse {
        id: String,
        name: String,
        #[serde(default)]
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        #[serde(default)]
        content: Value,
        #[serde(default)]
        is_error: bool,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum Content {
    Text(String),
    Blocks(Vec<Block>),
}

#[derive(Deserialize)]
pub(super) struct Body {
    pub id: Option<String>,
    pub content: Content,
}

#[derive(Deserialize)]
pub(super) struct Message {
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
pub(super) struct Finish {
    pub subtype: String,
    #[serde(default)]
    pub is_error: bool,
    pub result: Option<String>,
    pub terminal_reason: Option<String>,
    pub total_cost_usd: Option<f64>,
    pub usage: Option<RawUsage>,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Deserialize, Default)]
pub(super) struct RawUsage {
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
pub(super) struct ControlRequest {
    pub request_id: String,
    pub request: ControlBody,
}

#[derive(Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub(super) enum ControlBody {
    CanUseTool(Box<CanUseTool>),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct CanUseTool {
    pub tool_name: String,
    #[serde(default)]
    pub input: Value,
    pub tool_use_id: Option<String>,
    pub description: Option<String>,
    pub permission_suggestions: Option<Value>,
}
