//! Streamed lines: text, thinking and tool input as they arrive.

use std::time::Instant;

use serde_json::Value;

use crate::claude_code::{tools::self, wire::{Block, Delta, Stream, StreamEvent}};
use crate::session::{BlockId, Event, ToolId};

use super::super::structs::ClaudeLineMapper;
use super::super::enums::Open;

impl ClaudeLineMapper {
    pub(super) fn stream(&mut self, stream: Stream, now: Instant) -> Vec<Event> {
        let parent = stream.parent_tool_use_id.map(ToolId::new);
        match stream.event {
            StreamEvent::MessageStart { message } => {
                self.streamed.insert(message.id);
                Vec::new()
            }
            StreamEvent::ContentBlockStart { index, content_block } => match content_block {
                Block::Text { text } => {
                    let block = self.new_block();
                    self.open.insert(index, Open::Text(block));
                    if text.is_empty() { Vec::new() } else { vec![Event::Text { block, delta: text }] }
                }
                Block::Thinking { thinking } => {
                    let block = self.new_block();
                    self.open.insert(index, Open::Thinking(block, now));
                    vec![Event::Thinking { block, delta: thinking }]
                }
                Block::ToolUse { id, name, .. } => {
                    let deferred = tools::starts_subagent(&name) || tools::todo_tool(&name).is_some();
                    // A deferred call never names a file the review needs, so its input is not followed.
                    self.open.insert(index, Open::Tool { id: ToolId::new(&id), name: name.clone(), json: String::new(), targeted: deferred, shown: None, told: None });
                    if deferred {
                        return Vec::new();
                    }
                    self.announce(ToolId::new(id), name, Value::Null, parent)
                }
                Block::ToolResult { .. } | Block::Other => Vec::new(),
            },
            StreamEvent::ContentBlockDelta { index, delta } => match (self.open.get_mut(&index), delta) {
                (Some(Open::Tool { id, name, json, targeted, shown, told }), Delta::InputJson { partial_json }) if !*targeted || tools::streams_input(name) => {
                    json.push_str(&partial_json);
                    let mut events = Vec::new();
                    // A question is told as its text comes: what the card needs is in the input, which is whole only at the end.
                    if name == tools::ASK_QUESTION
                        && let Some(input) = crate::partial_json::value(json)
                        && told.as_ref() != Some(&input)
                    {
                        events.push(Event::ToolInput { id: id.clone(), input: input.clone(), file: None });
                        *told = Some(input);
                    }
                    if !*targeted && let Some(file) = tools::file_in_partial_input(json) {
                        *targeted = true;
                        events.push(Event::ToolTarget { id: id.clone(), file });
                    }
                    // An edit's text is told as it arrives, so the panel can show it being written, once its file is named.
                    if *targeted
                        && let Some(edit) = crate::partial_json::fields(json).and_then(|input| tools::edit_of(name, &input))
                        && shown.as_ref() != Some(&edit)
                    {
                        events.push(Event::ToolEdit { id: id.clone(), edit: edit.clone() });
                        *shown = Some(edit);
                    }
                    events
                }
                (Some(Open::Text(block)), Delta::Text { text }) if !text.is_empty() => {
                    vec![Event::Text { block: *block, delta: text }]
                }
                (Some(Open::Thinking(block, _)), Delta::Thinking { thinking }) if !thinking.is_empty() => {
                    vec![Event::Thinking { block: *block, delta: thinking }]
                }
                _ => Vec::new(),
            },
            StreamEvent::ContentBlockStop { index } => match self.open.remove(&index) {
                Some(Open::Thinking(block, since)) => {
                    vec![Event::ThinkingDone { block, took: now.saturating_duration_since(since) }]
                }
                _ => Vec::new(),
            },
            StreamEvent::Other => Vec::new(),
        }
    }

    pub(super) fn new_block(&mut self) -> BlockId {
        self.next_block += 1;
        BlockId(self.next_block)
    }
}
