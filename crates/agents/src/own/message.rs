//! The conversation in the words every model API shares: messages made of blocks. A model client maps
//! these to its API's JSON and back; nothing above it knows a wire format. The blocks are also what the
//! session record keeps, so a saved session resumes exactly.
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub blocks: Vec<Block>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self { role: Role::User, blocks: vec![Block::Text { text: text.into() }] }
    }

    pub fn assistant(blocks: Vec<Block>) -> Self {
        Self { role: Role::Assistant, blocks }
    }

    /// The message's plain text, blocks joined.
    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|b| if let Block::Text { text } = b { Some(text.as_str()) } else { None })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A tool as the model sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    /// A JSON schema object.
    pub schema: Value,
}

/// How much the model thinks. `Auto` lets the model decide (adaptive thinking); `Off` sends no thinking
/// setting. Models that always think ignore `Off`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Thinking {
    Off,
    #[default]
    Auto,
}

pub struct ModelRequest<'a> {
    pub model: &'a str,
    pub system: &'a str,
    pub tools: &'a [ToolDef],
    pub messages: &'a [Message],
    pub max_tokens: u32,
    pub thinking: Thinking,
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

/// Tokens one model call used.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

/// A whole reply.
#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    pub blocks: Vec<Block>,
    pub stop: StopReason,
    pub usage: TokenUsage,
    /// Ids of tool calls whose input was not valid JSON. Their `input` is `Null`.
    pub malformed: Vec<String>,
}

/// One step of a reply as it streams. Deltas of one block come in a row; [`Delta::BlockEnd`] closes it.
#[derive(Clone, Debug, PartialEq)]
pub enum Delta {
    Text(String),
    Thinking(String),
    ToolStart { id: String, name: String },
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

/// The model behind the agent: one streaming call. It knows messages and tools, and no project.
pub trait Model: Send + Sync {
    /// Streams the reply into `sink` and returns it whole. Checks `cancel` while it waits and returns
    /// [`ModelError::Cancelled`] soon after it is set.
    fn stream(&self, request: &ModelRequest<'_>, sink: &mut dyn FnMut(Delta), cancel: &Cancel) -> Result<Reply, ModelError>;
}

/// A flag one thread sets to stop work another thread does.
#[derive(Clone, Debug, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn set(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn clear(&self) {
        self.0.store(false, Ordering::SeqCst);
    }

    pub fn is_set(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// An API key. It prints as `***`, so a log line or an event that holds it shows nothing.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The key itself, for the one place that sends it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}
