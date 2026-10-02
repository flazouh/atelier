use serde::Deserialize;

use super::structs::{Chunk, Commands, ConfigOptions, Diff, Modes, Plan, ToolCall};

#[derive(Deserialize)]
#[serde(tag = "sessionUpdate", rename_all = "snake_case")]
pub(in super::super) enum SessionUpdate {
    UserMessageChunk(Chunk),
    AgentMessageChunk(Chunk),
    AgentThoughtChunk(Chunk),
    ToolCall(ToolCall),
    ToolCallUpdate(ToolCall),
    Plan(Plan),
    AvailableCommandsUpdate(Commands),
    CurrentModeUpdate(Modes),
    ConfigOptionUpdate(ConfigOptions),
    #[serde(other)]
    Other,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(in super::super) enum ContentBlock {
    Text {
        text: String,
    },
    #[serde(other)]
    Other,
}

impl ContentBlock {
    pub fn text(&self) -> &str {
        match self {
            Self::Text { text } => text,
            Self::Other => "",
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(in super::super) enum ToolContent {
    Content { content: ContentBlock },
    Diff(Diff),
    #[serde(other)]
    Other,
}
