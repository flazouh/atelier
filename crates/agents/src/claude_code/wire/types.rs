use serde::Deserialize;
use serde_json::Value;

use super::structs::{CanUseTool, ControlRequest, Finish, Message, Started, Stream, System};

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(in super::super) enum Line {
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

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(in super::super) enum StreamEvent {
    MessageStart { message: Started },
    ContentBlockStart { index: u32, content_block: Block },
    ContentBlockDelta { index: u32, delta: Delta },
    ContentBlockStop { index: u32 },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(in super::super) enum Delta {
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
pub(in super::super) enum Block {
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
pub(in super::super) enum Content {
    Text(String),
    Blocks(Vec<Block>),
}

#[derive(Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub(in super::super) enum ControlBody {
    CanUseTool(Box<CanUseTool>),
    #[serde(other)]
    Other,
}
