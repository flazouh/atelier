use serde_json::Value;

use super::super::message::{Block, Message};
use super::structs::{Budget, Compaction};
use super::types::{BIG, HEAD, MARK};

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

pub(super) fn shorten(text: &str) -> String {
    let mut end = HEAD.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{MARK}: {} bytes. It began:]\n{}", text.len(), &text[..end])
}

pub(super) fn shorten_strings(value: &mut Value) -> usize {
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
