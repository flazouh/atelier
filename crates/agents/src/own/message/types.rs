use std::{fmt, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Text {
        text: String,
    },
    /// The model's reasoning. Anthropic signs it, and needs it back unchanged in a turn that used a tool.
    Thinking {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    /// Reasoning the API returned encrypted. Sent back as it came.
    Redacted {
        data: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    /// In a user message: what a tool call returned.
    ToolResult {
        id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
}

/// How much the model thinks. `Auto` lets the model decide (adaptive thinking); `Off` sends no thinking
/// setting. Models that always think ignore `Off`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Thinking {
    Off,
    #[default]
    Auto,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    /// The model declined.
    Refusal,
    Other(String),
}

/// One step of a reply as it streams. Deltas of one block come in a row; [`Delta::BlockEnd`] closes it.
#[derive(Clone, Debug, PartialEq)]
pub enum Delta {
    Text(String),
    Thinking(String),
    ToolStart { id: String, name: String },
    /// The next piece of a call's input, as JSON text that is not whole yet.
    ToolInput { id: String, piece: String },
    /// A call's input is whole.
    ToolDone { id: String, input: Value },
    BlockEnd,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    /// The caller stopped the request.
    Cancelled,
    /// The key is missing or refused.
    Auth(String),
    RateLimited { retry_after: Option<Duration>, message: String },
    /// The service failed or is overloaded. Worth another try.
    Server { status: u16, message: String },
    /// The request itself is wrong: a bad field, a model that does not exist.
    Request { status: u16, message: String },
    Network(String),
    /// The stream does not read, or ended early.
    Malformed(String),
}

impl ModelError {
    /// Whether the same request may work a moment later.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::RateLimited { .. } | Self::Server { .. } | Self::Network(_))
    }
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("the request was stopped"),
            Self::Auth(why) => write!(f, "the API refused the key: {why}"),
            Self::RateLimited { message, .. } => write!(f, "the API is rate limiting requests: {message}"),
            Self::Server { status, message } => write!(f, "the API failed ({status}): {message}"),
            Self::Request { status, message } => write!(f, "the API rejected the request ({status}): {message}"),
            Self::Network(why) => write!(f, "the network failed: {why}"),
            Self::Malformed(why) => write!(f, "the reply did not read: {why}"),
        }
    }
}

impl std::error::Error for ModelError {}
