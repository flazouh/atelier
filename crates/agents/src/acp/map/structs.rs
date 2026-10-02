use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use serde_json::Value;

use super::super::wire::{self, PermissionAsked, SessionUpdate, ToolContent};
use crate::session::{
    BlockId, Choice, ChoiceId, Event, PermissionRequest, RequestId, ToolCall, ToolId, ToolKind,
    ToolOutput, ToolStatus, TurnEnd, TurnOutcome, Usage,
};
use super::types::Open;
use super::helpers::{
    choice_kind, content_text, diff_path, edit_of, file_of, open_status, output, status, todo, tool_kind,
};

/// A call as far as the updates have told it: the call atelier shows, and what it returned so far.
pub(super) struct Seen {
    pub(super) call: ToolCall,
    pub(super) content: Vec<ToolContent>,
    pub(super) raw_output: Option<Value>,
}

#[derive(Default)]
pub(in super::super) struct Mapper {
    next_block: u64,
    pub(super) open: Option<Open>,
    /// A user message a history replays, with the id its chunks share. Chunks join only under one id: a
    /// chunk with none is a message of its own, as Cursor sends each one whole.
    pub(super) user: Option<(Option<String>, String)>,
    pub(super) calls: HashMap<ToolId, Seen>,
    /// The calls in the order they started, so the ones a turn leaves open end in that order.
    order: Vec<ToolId>,
    /// The last text block of the turn: its closing text.
    last_text: String,
}

impl Mapper {
    /// The events for one update. Modes, commands and config options are the protocol's, not the
    /// conversation's, and give nothing here.
    pub fn update(&mut self, update: SessionUpdate, now: Instant) -> Vec<Event> {
        match update {
            SessionUpdate::UserMessageChunk(chunk) => {
                let mut events = self.close_block(now);
                match &mut self.user {
                    Some((Some(open), text)) if chunk.message_id.as_ref() == Some(open) => text.push_str(chunk.content.text()),
                    _ => {
                        events.extend(self.flush_user());
                        self.user = Some((chunk.message_id, chunk.content.text().to_string()));
                    }
                }
                events
            }
            SessionUpdate::AgentMessageChunk(chunk) => self.text(chunk.content.text(), now),
            SessionUpdate::AgentThoughtChunk(chunk) => self.thought(chunk.content.text(), now),
            SessionUpdate::ToolCall(call) => self.tool_call(call, now),
            SessionUpdate::ToolCallUpdate(update) => self.tool_update(update, now),
            SessionUpdate::Plan(plan) => {
                let mut events = self.settle(now);
                events.push(Event::Todos(plan.entries.into_iter().enumerate().map(|(i, entry)| todo(i, entry)).collect()));
                events
            }
            SessionUpdate::AvailableCommandsUpdate(_) | SessionUpdate::CurrentModeUpdate(_) | SessionUpdate::ConfigOptionUpdate(_) | SessionUpdate::Other => {
                Vec::new()
            }
        }
    }

    /// The events for the agent's `session/request_permission`: the call it is about, announced first when no
    /// update named it, then the question with the agent's options as choices. The content the question
    /// carries is its reason (Cursor's "Not in allowlist: rm"), not what the call returned.
    pub fn permission(&mut self, id: RequestId, mut asked: PermissionAsked, now: Instant) -> Vec<Event> {
        let reason = Some(content_text(&asked.tool_call.content.take().unwrap_or_default())).filter(|r| !r.is_empty());
        // Cursor sends the call as pending in its question while the updates already run it: the updates win.
        asked.tool_call.status = None;
        let call_id = ToolId::new(asked.tool_call.tool_call_id.clone());
        let mut events = if self.calls.contains_key(&call_id) {
            let events = self.settle(now);
            self.merge(&call_id, &asked.tool_call);
            events
        } else {
            self.tool_call(asked.tool_call, now)
        };
        let call = self.calls[&call_id].call.clone();
        let choices = asked
            .options
            .into_iter()
            .map(|option| Choice { id: ChoiceId::new(option.option_id), label: option.name, kind: choice_kind(&option.kind) })
            .collect();
        events.push(Event::Permission(PermissionRequest { id, call, reason, choices }));
        events
    }

    /// The events for the end of a turn: what streams closes, a call still open fails (the agent
    /// should have ended it, and the UI must not wait for it), then the usage and the end itself.
    pub fn turn_ended(&mut self, outcome: TurnOutcome, usage: Option<Usage>, now: Instant) -> Vec<Event> {
        let mut events = self.settle(now);
        let why = match outcome {
            TurnOutcome::Interrupted => "Interrupted before it finished.",
            _ => "The turn ended before this finished.",
        };
        events.extend(self.end_open_calls(why, true));
        events.extend(usage.map(Event::Usage));
        let summary = Some(std::mem::take(&mut self.last_text)).filter(|text| !text.is_empty());
        events.push(Event::TurnEnded(TurnEnd { outcome, summary }));
        events
    }

    /// The events for an agent that is gone: what streams closes and each open call fails with `why`.
    pub fn gone(&mut self, why: &str, now: Instant) -> Vec<Event> {
        let mut events = self.settle(now);
        events.extend(self.end_open_calls(why, true));
        events
    }

    /// The end of a history the agent replayed. A record has no end for a call that was still going when it
    /// stopped, and no live agent will send one, so it ends here rather than show as running forever.
    pub fn end_of_history(&mut self, now: Instant) -> Vec<Event> {
        let mut events = self.settle(now);
        events.extend(self.end_open_calls("The record has no end for this; it is shown as finished.", false));
        self.last_text.clear();
        events
    }

    pub(super) fn text(&mut self, delta: &str, now: Instant) -> Vec<Event> {
        if delta.is_empty() {
            return Vec::new();
        }
        let mut events = self.flush_user();
        let block = match self.open {
            Some(Open::Text(block)) => block,
            _ => {
                events.extend(self.close_block(now));
                self.last_text.clear();
                let block = self.block();
                self.open = Some(Open::Text(block));
                block
            }
        };
        self.last_text.push_str(delta);
        events.push(Event::Text { block, delta: delta.to_string() });
        events
    }

    /// A thought opens a thinking block, even an empty one: it says the agent thinks now. Its time runs
    /// until something else comes.
    fn thought(&mut self, delta: &str, now: Instant) -> Vec<Event> {
        let mut events = self.flush_user();
        match self.open {
            Some(Open::Thinking(block, _)) => {
                if !delta.is_empty() {
                    events.push(Event::Thinking { block, delta: delta.to_string() });
                }
            }
            _ => {
                events.extend(self.close_block(now));
                let block = self.block();
                self.open = Some(Open::Thinking(block, now));
                events.push(Event::Thinking { block, delta: delta.to_string() });
            }
        }
        events
    }

    fn tool_call(&mut self, update: wire::ToolCall, now: Instant) -> Vec<Event> {
        let id = ToolId::new(update.tool_call_id.clone());
        if self.calls.contains_key(&id) {
            return self.tool_update(update, now);
        }
        let mut events = self.settle(now);
        let content = update.content.clone().unwrap_or_default();
        let call = ToolCall {
            id: id.clone(),
            name: update.title.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "Tool".into()),
            kind: tool_kind(update.kind.as_deref(), &content),
            input: update.raw_input.clone().unwrap_or(Value::Null),
            file: file_of(&update),
            parent: None,
            status: status(update.status.as_deref()).unwrap_or(ToolStatus::Pending),
        };
        events.push(Event::ToolStarted(ToolCall { status: open_status(call.status), ..call.clone() }));
        if let Some(edit) = edit_of(&content) {
            events.push(Event::ToolEdit { id: id.clone(), edit });
        }
        self.order.push(id.clone());
        self.calls.insert(id.clone(), Seen { call, content, raw_output: update.raw_output });
        if let Some(end) = self.finish_if_ended(&id) {
            events.push(end);
        }
        events
    }

    fn tool_update(&mut self, update: wire::ToolCall, now: Instant) -> Vec<Event> {
        let id = ToolId::new(update.tool_call_id.clone());
        if !self.calls.contains_key(&id) {
            return self.tool_call(update, now);
        }
        let mut events = self.settle(now);
        let before = self.calls[&id].call.clone();
        let edit_before = edit_of(&self.calls[&id].content);
        self.merge(&id, &update);
        let edit = edit_of(&self.calls[&id].content).filter(|edit| Some(edit) != edit_before.as_ref());
        let after = &self.calls[&id].call;
        if after.input != before.input {
            events.push(Event::ToolInput { id: id.clone(), input: after.input.clone(), file: after.file.clone() });
        } else if let (None, Some(file)) = (&before.file, &after.file) {
            events.push(Event::ToolTarget { id: id.clone(), file: file.clone() });
        }
        if after.kind != before.kind {
            events.push(Event::ToolKind { id: id.clone(), kind: after.kind });
        }
        if let Some(edit) = edit {
            events.push(Event::ToolEdit { id: id.clone(), edit });
        }
        match after.status {
            ToolStatus::Done | ToolStatus::Failed if before.status != after.status => events.extend(self.finish_if_ended(&id)),
            ToolStatus::Running if before.status != ToolStatus::Running => {
                events.push(Event::ToolStatus { id, status: ToolStatus::Running });
            }
            _ => {}
        }
        events
    }

    /// Folds what an update says into the call it names. Content replaces what came before, as ACP says.
    fn merge(&mut self, id: &ToolId, update: &wire::ToolCall) {
        let Some(seen) = self.calls.get_mut(id) else { return };
        if let Some(content) = &update.content {
            seen.content = content.clone();
        }
        if let Some(raw) = &update.raw_output {
            seen.raw_output = Some(raw.clone());
        }
        if let Some(input) = update.raw_input.clone().filter(|input| !input.is_null()) {
            seen.call.input = input;
        }
        if seen.call.file.is_none() {
            seen.call.file = file_of(update).or_else(|| diff_path(&seen.content));
        }
        if update.kind.is_some() {
            seen.call.kind = tool_kind(update.kind.as_deref(), &seen.content);
        } else if seen.call.kind == ToolKind::Edit && update.content.is_some() {
            seen.call.kind = tool_kind(Some("edit"), &seen.content);
        }
        if let Some(status) = status(update.status.as_deref()) {
            seen.call.status = status;
        }
    }

    /// `ToolFinished` for a call whose status says it has ended.
    fn finish_if_ended(&self, id: &ToolId) -> Option<Event> {
        let seen = self.calls.get(id)?;
        let failed = match seen.call.status {
            ToolStatus::Done => false,
            ToolStatus::Failed => true,
            ToolStatus::Pending | ToolStatus::Running => return None,
        };
        Some(Event::ToolFinished { id: id.clone(), output: output(seen, failed) })
    }

    fn end_open_calls(&mut self, why: &str, failed: bool) -> Vec<Event> {
        let mut events = Vec::new();
        for id in &self.order {
            let Some(seen) = self.calls.get_mut(id) else { continue };
            if matches!(seen.call.status, ToolStatus::Pending | ToolStatus::Running) {
                seen.call.status = if failed { ToolStatus::Failed } else { ToolStatus::Done };
                let output = ToolOutput { text: why.to_string(), is_error: failed, truncated: false, full_at: None };
                events.push(Event::ToolFinished { id: id.clone(), output });
            }
        }
        events
    }

    /// Ends what streams and the user text a history is joining, before anything that is not part of them.
    fn settle(&mut self, now: Instant) -> Vec<Event> {
        let mut events = self.flush_user();
        events.extend(self.close_block(now));
        events
    }

    fn flush_user(&mut self) -> Vec<Event> {
        match self.user.take() {
            Some((_, text)) if !text.is_empty() => vec![Event::UserMessage { text }],
            _ => Vec::new(),
        }
    }

    fn close_block(&mut self, now: Instant) -> Vec<Event> {
        match self.open.take() {
            Some(Open::Thinking(block, since)) => {
                vec![Event::ThinkingDone { block, took: now.checked_duration_since(since).unwrap_or(Duration::ZERO) }]
            }
            Some(Open::Text(_)) | None => Vec::new(),
        }
    }

    pub(super) fn block(&mut self) -> BlockId {
        self.next_block += 1;
        BlockId(self.next_block)
    }
}
