//! What the panel shows, built from events. The fold is pure: give it the events of a session in order
//! and it holds the items to draw, the todo list, the usage and whether the agent works. The UI keeps one
//! `Conversation` per session and calls `apply` for each event a frame drains, so drawing never depends on
//! which backend made the events.
use std::{collections::HashMap, time::Duration};

use super::event::{
    BlockId, ChoiceKind, EndReason, Event, PermissionRequest, RequestId, Started, Subagent, Todo, ToolCall, ToolId,
    ToolOutput, ToolStatus, TurnEnd, TurnOutcome, Usage,
};

/// A tool call and what it returned.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub call: ToolCall,
    pub output: Option<ToolOutput>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubagentStatus {
    Running,
    Done,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The question waits for the user.
    Asking,
    Answered(ChoiceKind),
    /// The agent withdrew it, or the turn ended first.
    Withdrawn,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    User { text: String },
    Text { block: BlockId, text: String },
    Thinking { block: BlockId, text: String, took: Option<Duration> },
    Tool(Call),
    Subagent {
        subagent: Subagent,
        status: SubagentStatus,
        /// What it does now.
        activity: Option<String>,
        summary: Option<String>,
        /// Its own tool calls, in order.
        calls: Vec<Call>,
    },
    Permission { request: PermissionRequest, answer: Answer },
    /// A problem the user should see: a warning, or a turn that failed.
    Notice(String),
}

/// Where a tool call lives: at the top level, or inside a subagent.
#[derive(Clone, Copy, Debug)]
enum Slot {
    Top(usize),
    Sub(usize, usize),
}

#[derive(Debug, Default)]
pub struct Conversation {
    items: Vec<Item>,
    todos: Vec<Todo>,
    usage: Usage,
    working: bool,
    started: Option<Started>,
    ended: Option<EndReason>,
    last_turn: Option<TurnEnd>,
    calls: HashMap<ToolId, Slot>,
    subagents: HashMap<ToolId, usize>,
}

impl Conversation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn todos(&self) -> &[Todo] {
        &self.todos
    }

    /// The tokens and cost of every turn so far.
    pub fn usage(&self) -> Usage {
        self.usage
    }

    /// Whether a turn is open: the agent works, or waits for an answer.
    pub fn working(&self) -> bool {
        self.working
    }

    pub fn started(&self) -> Option<&Started> {
        self.started.as_ref()
    }

    pub fn ended(&self) -> Option<&EndReason> {
        self.ended.as_ref()
    }

    pub fn last_turn(&self) -> Option<&TurnEnd> {
        self.last_turn.as_ref()
    }

    /// How many subagents run now.
    pub fn running_subagents(&self) -> usize {
        self.items.iter().filter(|item| matches!(item, Item::Subagent { status: SubagentStatus::Running, .. })).count()
    }

    /// atelier sent a message: it shows at once and a turn opens.
    pub fn user_sent(&mut self, text: impl Into<String>) {
        self.items.push(Item::User { text: text.into() });
        self.working = true;
    }

    /// The user picked a choice. The card shows it before the agent's next event arrives.
    pub fn answered(&mut self, request: &RequestId, kind: ChoiceKind) {
        if let Some(answer) = self.items.iter_mut().rev().find_map(|item| match item {
            Item::Permission { request: asked, answer } if asked.id == *request => Some(answer),
            _ => None,
        }) {
            *answer = Answer::Answered(kind);
        }
    }

    pub fn apply(&mut self, event: &Event) {
        match event {
            Event::Started(started) => self.started = Some(started.clone()),
            Event::UserMessage { text } => self.items.push(Item::User { text: text.clone() }),
            Event::Text { block, delta } => match self.items.last_mut() {
                Some(Item::Text { block: open, text }) if open == block => text.push_str(delta),
                _ => self.items.push(Item::Text { block: *block, text: delta.clone() }),
            },
            Event::Thinking { block, delta } => match self.items.last_mut() {
                Some(Item::Thinking { block: open, text, .. }) if open == block => text.push_str(delta),
                _ => self.items.push(Item::Thinking { block: *block, text: delta.clone(), took: None }),
            },
            Event::ThinkingDone { block, took } => {
                if let Some(time) = self.items.iter_mut().rev().find_map(|item| match item {
                    Item::Thinking { block: open, took, .. } if open == block => Some(took),
                    _ => None,
                }) {
                    *time = Some(*took);
                }
            }
            Event::ToolStarted(call) => self.tool_started(call),
            Event::ToolTarget { id, file } => {
                if let Some(call) = self.call_mut(id) {
                    call.call.file.get_or_insert_with(|| file.clone());
                }
            }
            Event::ToolInput { id, input, file } => {
                if let Some(call) = self.call_mut(id) {
                    call.call.file = file.clone().or_else(|| call.call.file.take());
                    call.call.input = input.clone();
                }
            }
            Event::ToolStatus { id, status } => {
                if let Some(call) = self.call_mut(id) {
                    call.call.status = *status;
                }
            }
            Event::ToolKind { id, kind } => {
                if let Some(call) = self.call_mut(id) {
                    call.call.kind = *kind;
                }
            }
            Event::ToolFinished { id, output } => {
                if let Some(call) = self.call_mut(id) {
                    call.call.status = if output.is_error { ToolStatus::Failed } else { ToolStatus::Done };
                    call.output = Some(output.clone());
                }
            }
            Event::SubagentStarted(subagent) => {
                self.subagents.insert(subagent.id.clone(), self.items.len());
                self.items.push(Item::Subagent {
                    subagent: subagent.clone(),
                    status: SubagentStatus::Running,
                    activity: None,
                    summary: None,
                    calls: Vec::new(),
                });
            }
            Event::SubagentProgress { id, activity: now } => {
                if let Some(Item::Subagent { activity, .. }) = self.subagent_mut(id) {
                    *activity = Some(now.clone());
                }
            }
            Event::SubagentEnded { id, ok, summary: said } => {
                if let Some(Item::Subagent { status, activity, summary, .. }) = self.subagent_mut(id) {
                    *status = if *ok { SubagentStatus::Done } else { SubagentStatus::Failed };
                    *activity = None;
                    *summary = said.clone();
                }
            }
            Event::Todos(todos) => self.todos = todos.clone(),
            Event::Permission(request) => {
                self.items.push(Item::Permission { request: request.clone(), answer: Answer::Asking });
            }
            Event::PermissionCancelled(id) => {
                if let Some(answer) = self.items.iter_mut().rev().find_map(|item| match item {
                    Item::Permission { request, answer } if request.id == *id && *answer == Answer::Asking => Some(answer),
                    _ => None,
                }) {
                    *answer = Answer::Withdrawn;
                }
            }
            Event::Usage(usage) => {
                self.usage.input_tokens += usage.input_tokens;
                self.usage.output_tokens += usage.output_tokens;
                self.usage.cache_read_tokens += usage.cache_read_tokens;
                self.usage.cache_write_tokens += usage.cache_write_tokens;
                self.usage.cost_usd = match (self.usage.cost_usd, usage.cost_usd) {
                    (None, None) => None,
                    (a, b) => Some(a.unwrap_or(0.) + b.unwrap_or(0.)),
                };
            }
            Event::TurnEnded(end) => {
                self.working = false;
                if let TurnOutcome::Failed(why) = &end.outcome {
                    self.items.push(Item::Notice(why.clone()));
                }
                self.last_turn = Some(end.clone());
            }
            Event::Warning(text) => self.items.push(Item::Notice(text.clone())),
            Event::Ended(reason) => {
                self.working = false;
                self.ended = Some(reason.clone());
            }
        }
    }

    fn tool_started(&mut self, call: &ToolCall) {
        let inside = call.parent.as_ref().and_then(|parent| self.subagents.get(parent).copied());
        let entry = Call { call: call.clone(), output: None };
        match inside {
            Some(item) => {
                let Item::Subagent { calls, .. } = &mut self.items[item] else { return };
                self.calls.insert(call.id.clone(), Slot::Sub(item, calls.len()));
                calls.push(entry);
            }
            None => {
                self.calls.insert(call.id.clone(), Slot::Top(self.items.len()));
                self.items.push(Item::Tool(entry));
            }
        }
    }

    fn call_mut(&mut self, id: &ToolId) -> Option<&mut Call> {
        match *self.calls.get(id)? {
            Slot::Top(at) => match self.items.get_mut(at)? {
                Item::Tool(call) => Some(call),
                _ => None,
            },
            Slot::Sub(item, at) => match self.items.get_mut(item)? {
                Item::Subagent { calls, .. } => calls.get_mut(at),
                _ => None,
            },
        }
    }

    fn subagent_mut(&mut self, id: &ToolId) -> Option<&mut Item> {
        let at = *self.subagents.get(id)?;
        self.items.get_mut(at)
    }
}
