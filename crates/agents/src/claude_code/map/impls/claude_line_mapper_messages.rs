//! Finished messages: the agent's, and the reader's in a transcript.

use std::time::Duration;

use crate::claude_code::wire::{Block, Content, Message};
use crate::session::{ContextFill, Event, ToolId};

use super::{
    super::{consts::SYNTHETIC_MODEL, structs::ClaudeLineMapper},
    claude_line_mapper_context::{context_tokens, known_window},
};

impl ClaudeLineMapper {
    /// A finished assistant message: what streamed is already told, what did not stream is told now,
    /// and every tool call gets its whole input.
    pub(super) fn assistant(&mut self, message: Message) -> Vec<Event> {
        if message.sidechain {
            return Vec::new();
        }
        // Its text tells the reader to run `/login`, which a headless run cannot: the notice says what to do.
        if message.is_signed_out() {
            return vec![Event::SignedOut];
        }
        let parent = message.parent_tool_use_id.map(ToolId::new);
        let streamed = message.message.id.as_ref().is_some_and(|id| self.streamed.contains(id));
        let synthetic = parent.is_none() && message.message.model.as_deref() == Some(SYNTHETIC_MODEL);
        if synthetic {
            if let Content::Blocks(blocks) = message.message.content {
                self.held.extend(blocks.into_iter().filter_map(|block| match block {
                    Block::Text { text } if !text.is_empty() => Some(text),
                    _ => None,
                }));
            }
            return Vec::new();
        }
        let mut events = self.flush_held();
        if parent.is_none() {
            if let Some(model) = message.message.model {
                self.model = Some(model);
            }
            if let Some(usage) = &message.message.usage {
                let window = self.model.as_deref().and_then(known_window).or(self.context.window);
                events.extend(self.context_changed(ContextFill { used: context_tokens(usage), window }));
            }
        }
        let Content::Blocks(blocks) = message.message.content else { return events };
        for block in blocks {
            match block {
                Block::Text { text } if !streamed && !text.is_empty() => {
                    let block = self.new_block();
                    events.push(Event::Text { block, delta: text });
                }
                Block::Thinking { thinking } if !streamed && !thinking.is_empty() => {
                    let block = self.new_block();
                    events.push(Event::Thinking { block, delta: thinking });
                    events.push(Event::ThinkingDone { block, took: Duration::ZERO });
                }
                Block::ToolUse { id, name, input } => {
                    events.extend(self.tool_use(ToolId::new(id), name, input, parent.clone()));
                }
                _ => {}
            }
        }
        events
    }

    pub(super) fn user(&mut self, message: Message) -> Vec<Event> {
        if message.sidechain || message.written_by_claude() {
            return Vec::new();
        }
        let mut result = message.tool_use_result;
        match message.message.content {
            Content::Text(text) => self.user_text(text, message.parent_tool_use_id.is_some()),
            Content::Blocks(blocks) => {
                let mut events = Vec::new();
                for block in blocks {
                    match block {
                        Block::ToolResult { tool_use_id, content, is_error } => {
                            events.extend(self.tool_result(ToolId::new(tool_use_id), &content, is_error, result.take()));
                        }
                        Block::Text { text } => events.extend(self.user_text(text, message.parent_tool_use_id.is_some())),
                        _ => {}
                    }
                }
                events
            }
        }
    }

    /// The held replies `claude` wrote itself, as text, in order.
    pub(super) fn flush_held(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.held).into_iter().map(|text| Event::Text { block: self.new_block(), delta: text }).collect()
    }

    /// A user message that did not come from atelier: history. `claude` also writes a line for an
    /// interrupt and for a subagent's prompt; neither is something the user said.
    pub(super) fn user_text(&mut self, text: String, from_subagent: bool) -> Vec<Event> {
        if from_subagent || text.starts_with("[Request interrupted") || text.trim().is_empty() {
            return Vec::new();
        }
        vec![Event::UserMessage { text }]
    }
}
