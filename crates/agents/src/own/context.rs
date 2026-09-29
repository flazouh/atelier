//! The context budget. A long session fills the model's window with old tool output, which the model
//! rarely needs again. When the conversation grows past the budget, the oldest tool results (and the big
//! strings in old tool inputs, such as a file that was written) are shortened to a note and their first
//! lines, oldest first, until it fits with room to spare. The messages and the order of calls stay, so the
//! conversation still reads. A shortened message changes the prompt cache from that point, so this runs
//! only when the budget is passed, and shortens well below it so it does not run every turn.
use serde_json::Value;

use super::message::{Block, Message};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// Estimated tokens the conversation may hold.
    pub limit_tokens: usize,
    /// The newest messages are never shortened.
    pub keep_recent: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self { limit_tokens: 120_000, keep_recent: 8 }
    }
}

/// What a compaction did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Compaction {
    pub shortened: usize,
    pub saved_bytes: usize,
}

/// A string this long in an old message is worth shortening.
const BIG: usize = 600;
/// How much of a shortened text stays.
const HEAD: usize = 240;

/// The conversation's size in tokens, estimated as four bytes to a token. It is a guide, not a count.
pub fn estimate(messages: &[Message]) -> usize {
    let bytes: usize = messages
        .iter()
        .map(|m| {
            m.blocks
                .iter()
                .map(|b| match b {
                    Block::Text { text } => text.len(),
                    Block::Thinking { text, signature } => text.len() + signature.as_ref().map_or(0, String::len),
                    Block::Redacted { data } => data.len(),
                    Block::ToolUse { name, input, .. } => name.len() + input.to_string().len(),
                    Block::ToolResult { content, .. } => content.len(),
                })
                .sum::<usize>()
                + 16
        })
        .sum();
    bytes / 4
}

const MARK: &str = "[omitted to save space";

fn shorten(text: &str) -> String {
    let mut end = HEAD.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{MARK}: {} bytes. It began:]\n{}", text.len(), &text[..end])
}

fn shorten_strings(value: &mut Value) -> usize {
    match value {
        Value::String(s) if s.len() > BIG && !s.starts_with(MARK) => {
            let saved = s.len();
            *s = shorten(s);
            saved.saturating_sub(s.len())
        }
        Value::Object(map) => map.values_mut().map(shorten_strings).sum(),
        Value::Array(items) => items.iter_mut().map(shorten_strings).sum(),
        _ => 0,
    }
}

/// Shortens old results and inputs when `messages` pass the budget. Goes on until the conversation is
/// under 80% of the limit or nothing old and big is left.
pub fn compact(messages: &mut [Message], budget: &Budget) -> Compaction {
    let mut result = Compaction::default();
    let mut tokens = estimate(messages);
    if tokens <= budget.limit_tokens {
        return result;
    }
    let target = budget.limit_tokens / 5 * 4;
    let protected_from = messages.len().saturating_sub(budget.keep_recent);
    'messages: for message in messages.iter_mut().take(protected_from) {
        for block in &mut message.blocks {
            let saved = match block {
                Block::ToolResult { content, .. } if content.len() > BIG && !content.starts_with(MARK) => {
                    let before = content.len();
                    *content = shorten(content);
                    before.saturating_sub(content.len())
                }
                Block::ToolUse { input, .. } => shorten_strings(input),
                _ => 0,
            };
            if saved > 0 {
                result.shortened += 1;
                result.saved_bytes += saved;
                tokens = tokens.saturating_sub(saved / 4);
                if tokens <= target {
                    break 'messages;
                }
            }
        }
    }
    result
}
