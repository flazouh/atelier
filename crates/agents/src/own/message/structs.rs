use std::{
    fmt,
    sync::{Arc, atomic::{AtomicBool, Ordering}},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::types::{Block, Role, StopReason, Thinking};

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

pub struct ModelRequest<'a> {
    pub model: &'a str,
    pub system: &'a str,
    pub tools: &'a [ToolDef],
    pub messages: &'a [Message],
    pub max_tokens: u32,
    pub thinking: Thinking,
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

/// A flag one thread sets to stop work another thread does.
#[derive(Clone, Debug, Default)]
pub struct Cancel(pub(super) Arc<AtomicBool>);

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
pub struct Secret(pub(super) String);

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
