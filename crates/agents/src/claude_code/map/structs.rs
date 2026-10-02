use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use serde_json::Value;

use super::super::{
    control::{self, mode_from_name},
    tools::{self, TodoTool},
    wire::{Block, CanUseTool, Content, ControlBody, ControlRequest, Delta, Finish, Line, Message, Stream, StreamEvent, System},
};
use crate::{
    session::{BlockId, Choice, ChoiceId, ChoiceKind, EndReason, Event, PermissionRequest, RequestId, SessionId, Started, Subagent, Todo, TodoStatus, ToolCall, ToolId, ToolOutput, ToolStatus, TurnEnd, TurnOutcome, Usage},
    subprocess,
};
use super::types::{ALLOW, ALLOW_ALWAYS, DENY, Open};
use super::helpers::{flatten, is_agent_task, task_number, todo_status, tool_output};

/// A question `claude` asked and atelier has not answered.
struct Asked {
    pub(super) input: Value,
    suggestions: Option<Value>,
}

#[derive(Default)]
pub struct Mapper {
    next_block: u64,
    /// Messages that streamed: their finished copy repeats what the deltas already said.
    streamed: HashSet<String>,
    open: HashMap<u32, Open>,
    /// Calls announced and not finished.
    running: HashSet<ToolId>,
    /// Calls whose result atelier swallows: todo edits and subagent starts.
    hidden: HashSet<ToolId>,
    subagents: HashSet<ToolId>,
    /// Shell commands that run on after their call returned (`run_in_background`), until `claude` says
    /// they ended. Each is a shell call that stays running, not a subagent.
    background: HashSet<ToolId>,
    /// The last turn ended aborted.
    aborted: bool,
    todos: Vec<Todo>,
    /// `TaskCreate` calls waiting for the id the result gives the task.
    creating: HashMap<ToolId, String>,
    asked: HashMap<RequestId, Asked>,
    turn_open: bool,
    ended: bool,
}

impl Mapper {
    pub fn new() -> Self {
        Self::default()
    }

    /// atelier sent a user message: a turn is open until `claude` reports its result.
    pub fn user_sent(&mut self) {
        self.turn_open = true;
    }

    /// Reads one line of `claude`'s stdout. A line that is not JSON gives a warning; a line of a
    /// kind atelier does not know gives nothing.
    pub fn line(&mut self, line: &str, now: Instant) -> Vec<Event> {
        let line = line.trim();
        if line.is_empty() {
            return Vec::new();
        }
        match serde_json::from_str::<Line>(line) {
            Ok(parsed) => self.parsed(parsed, now),
            Err(error) => vec![Event::Warning(format!("a line from the agent did not parse: {error}"))],
        }
    }

    fn parsed(&mut self, line: Line, now: Instant) -> Vec<Event> {
        match line {
            Line::System(system) => self.system(system),
            Line::StreamEvent(stream) => self.stream(stream, now),
            Line::Assistant(message) => self.assistant(message),
            Line::User(message) => self.user(message),
            Line::Finished(finish) => self.finished(finish),
            Line::ControlRequest(request) => self.control_request(request),
            Line::ControlCancelRequest { request_id } => {
                let id = RequestId::new(request_id);
                if self.asked.remove(&id).is_some() { vec![Event::PermissionCancelled(id)] } else { Vec::new() }
            }
            Line::Ignored => Vec::new(),
        }
    }

    /// The line to write for a permission answer, or `None` when the request is unknown or already
    /// answered.
    pub fn answer(&mut self, request: &RequestId, choice: &ChoiceId) -> Option<String> {
        let asked = self.asked.remove(request)?;
        Some(match choice.as_str() {
            ALLOW => control::allow(request.as_str(), &asked.input, None),
            ALLOW_ALWAYS => control::allow(request.as_str(), &asked.input, asked.suggestions.as_ref()),
            _ => control::deny(request.as_str()),
        })
    }

    /// The events for a process that ended: `code` is its exit code, `None` when a signal ended it. A
    /// turn or a tool still open fails, so the UI never waits for an agent that is gone. A session ends
    /// once: a second call to `exited` or `closed` gives nothing.
    pub fn exited(&mut self, code: Option<i32>, stderr: &str) -> Vec<Event> {
        if std::mem::replace(&mut self.ended, true) {
            return Vec::new();
        }
        let tail = subprocess::stderr_tail(stderr);
        let why = subprocess::exit_why(code, &tail);
        let mut events = self.fail_open_tools(&why, false);
        events.extend(self.asked.drain().map(|(id, _)| Event::PermissionCancelled(id)));
        let mut open: Vec<_> = self.subagents.drain().collect();
        open.sort();
        events.extend(open.into_iter().map(|id| Event::SubagentEnded { id, ok: false, summary: Some(why.clone()) }));
        if std::mem::take(&mut self.turn_open) {
            events.push(Event::TurnEnded(TurnEnd { outcome: TurnOutcome::Failed(why), summary: None }));
        }
        events.push(Event::Ended(EndReason::Exited { code, stderr: tail }));
        events
    }

    /// The end of a transcript that is read as history. A record has no end for a subagent or a background
    /// command that was still going when it stopped being written, and no live agent will ever send one, so
    /// each of them ends here rather than show as running forever. If the last turn ended aborted they
    /// end as interrupted; otherwise as finished, with a note that the record does not say how it went.
    pub fn end_of_history(&mut self) -> Vec<Event> {
        let (ok, note) = if self.aborted {
            (false, "Interrupted before it finished.")
        } else {
            (true, "The record has no end for this; it is shown as finished.")
        };
        let mut open: Vec<_> = self.running.drain().collect();
        open.sort();
        let mut events: Vec<Event> = open
            .into_iter()
            .map(|id| Event::ToolFinished { id, output: ToolOutput { text: note.into(), is_error: !ok, truncated: false, full_at: None } })
            .collect();
        let mut subagents: Vec<_> = self.subagents.drain().collect();
        subagents.sort();
        events.extend(subagents.into_iter().map(|id| Event::SubagentEnded { id, ok, summary: Some(note.into()) }));
        events.extend(self.asked.drain().map(|(id, _)| Event::PermissionCancelled(id)));
        self.background.clear();
        events
    }
    /// The event for a session atelier closed on purpose: it ends, and nothing else fails.
    pub fn closed(&mut self) -> Vec<Event> {
        if std::mem::replace(&mut self.ended, true) { Vec::new() } else { vec![Event::Ended(EndReason::Closed)] }
    }

    fn system(&mut self, system: System) -> Vec<Event> {
        match system.subtype.as_str() {
            "init" => match system.session_id {
                Some(id) => vec![Event::Started(Started {
                    session: SessionId::new(id),
                    model: system.model,
                    mode: system.permission_mode.as_deref().and_then(mode_from_name),
                    commands: system.slash_commands,
                })],
                None => Vec::new(),
            },
            "task_started" => {
                let Some(id) = system.tool_use_id.map(ToolId::new) else { return Vec::new() };
                // Only an agent task is a subagent. A background shell (`local_bash`), and any other kind of
                // task, is the call that started it, still running.
                if !is_agent_task(system.task_type.as_deref()) {
                    self.background.insert(id);
                    return Vec::new();
                }
                if self.subagents.insert(id.clone()) {
                    self.hidden.insert(id.clone());
                    let subagent = Subagent {
                        id,
                        task: system.description.unwrap_or_default(),
                        kind: system.subagent_type,
                        model: None,
                    };
                    return vec![Event::SubagentStarted(subagent)];
                }
                Vec::new()
            }
            "task_progress" => match (system.tool_use_id, system.description) {
                (Some(id), Some(activity)) => vec![Event::SubagentProgress { id: ToolId::new(id), activity }],
                _ => Vec::new(),
            },
            "task_notification" => {
                let Some(id) = system.tool_use_id.map(ToolId::new) else { return Vec::new() };
                if self.background.remove(&id) {
                    // The call returned long ago; now it ends, with what `claude` says of it.
                    if !self.running.remove(&id) {
                        return Vec::new();
                    }
                    let ok = system.status.as_deref() == Some("completed");
                    let summary = system.summary.unwrap_or_else(|| if ok { "The command finished.".into() } else { "The command did not finish well.".into() });
                    return vec![Event::ToolFinished { id, output: tool_output(&summary, !ok) }];
                }
                self.subagents.remove(&id);
                vec![Event::SubagentEnded { id, ok: system.status.as_deref() == Some("completed"), summary: system.summary }]
            }
            _ => Vec::new(),
        }
    }

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
                    self.open.insert(index, Open::Tool { id: ToolId::new(&id), name: name.clone(), json: String::new(), targeted: deferred, shown: None });
                    if deferred {
                        return Vec::new();
                    }
                    self.announce(ToolId::new(id), name, Value::Null, parent)
                }
                Block::ToolResult { .. } | Block::Other => Vec::new(),
            },
            StreamEvent::ContentBlockDelta { index, delta } => match (self.open.get_mut(&index), delta) {
                (Some(Open::Tool { id, name, json, targeted, shown }), Delta::InputJson { partial_json }) if !*targeted || tools::streams_input(name) => {
                    json.push_str(&partial_json);
                    let mut events = Vec::new();
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

    /// A finished assistant message: what streamed is already told, what did not stream is told now,
    /// and every tool call gets its whole input.
    fn assistant(&mut self, message: Message) -> Vec<Event> {
        if message.sidechain {
            return Vec::new();
        }
        let parent = message.parent_tool_use_id.map(ToolId::new);
        let streamed = message.message.id.as_ref().is_some_and(|id| self.streamed.contains(id));
        let Content::Blocks(blocks) = message.message.content else { return Vec::new() };
        let mut events = Vec::new();
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

    fn user(&mut self, message: Message) -> Vec<Event> {
        if message.sidechain || message.meta {
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

    /// A user message that did not come from atelier: history. `claude` also writes a line for an
    /// interrupt and for a subagent's prompt; neither is something the user said.
    fn user_text(&mut self, text: String, from_subagent: bool) -> Vec<Event> {
        if from_subagent || text.starts_with("[Request interrupted") || text.trim().is_empty() {
            return Vec::new();
        }
        vec![Event::UserMessage { text }]
    }

    fn finished(&mut self, finish: Finish) -> Vec<Event> {
        let interrupted = matches!(finish.terminal_reason.as_deref(), Some("aborted_tools" | "aborted_streaming"));
        let outcome = if interrupted {
            TurnOutcome::Interrupted
        } else if finish.subtype == "success" && !finish.is_error {
            TurnOutcome::Completed
        } else {
            let why = if finish.errors.is_empty() { finish.result.clone().unwrap_or(finish.subtype) } else { finish.errors.join("; ") };
            TurnOutcome::Failed(why)
        };
        self.turn_open = false;
        self.aborted = interrupted;
        let mut events = self.fail_open_tools("the turn ended before the tool finished", true);
        events.extend(self.asked.drain().map(|(id, _)| Event::PermissionCancelled(id)));
        if let Some(usage) = finish.usage {
            events.push(Event::Usage(Usage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cache_read_tokens: usage.cache_read_input_tokens,
                cache_write_tokens: usage.cache_creation_input_tokens,
                cost_usd: finish.total_cost_usd,
            }));
        }
        events.push(Event::TurnEnded(TurnEnd { outcome, summary: finish.result.filter(|text| !text.is_empty()) }));
        events
    }

    fn control_request(&mut self, request: ControlRequest) -> Vec<Event> {
        let ControlBody::CanUseTool(ask) = request.request else { return Vec::new() };
        let CanUseTool { tool_name, input, tool_use_id, description, permission_suggestions } = *ask;
        let id = RequestId::new(request.request_id);
        let has_rules = permission_suggestions.as_ref().is_some_and(|rules| rules.as_array().is_none_or(|a| !a.is_empty()));
        let mut choices = vec![Choice { id: ChoiceId::new(ALLOW), label: "Allow".into(), kind: ChoiceKind::Allow }];
        if has_rules {
            choices.push(Choice { id: ChoiceId::new(ALLOW_ALWAYS), label: "Always allow".into(), kind: ChoiceKind::AllowAlways });
        }
        choices.push(Choice { id: ChoiceId::new(DENY), label: "Deny".into(), kind: ChoiceKind::Deny });
        let call = ToolCall {
            id: ToolId::new(tool_use_id.unwrap_or_else(|| id.as_str().to_string())),
            kind: tools::kind(&tool_name),
            file: tools::file(&input),
            name: tool_name,
            input: input.clone(),
            parent: None,
            status: ToolStatus::Pending,
        };
        self.asked.insert(id.clone(), Asked { input, suggestions: permission_suggestions.filter(|_| has_rules) });
        vec![Event::Permission(PermissionRequest { id, call, reason: description.filter(|d| !d.is_empty()), choices })]
    }

    fn new_block(&mut self) -> BlockId {
        self.next_block += 1;
        BlockId(self.next_block)
    }

    fn announce(&mut self, id: ToolId, name: String, input: Value, parent: Option<ToolId>) -> Vec<Event> {
        self.running.insert(id.clone());
        vec![Event::ToolStarted(ToolCall {
            id,
            kind: tools::kind(&name),
            file: tools::file(&input),
            name,
            input,
            parent,
            status: ToolStatus::Running,
        })]
    }

    /// A tool call with its whole input, from a finished message.
    fn tool_use(&mut self, id: ToolId, name: String, input: Value, parent: Option<ToolId>) -> Vec<Event> {
        if tools::starts_subagent(&name) {
            self.hidden.insert(id.clone());
            if !self.subagents.insert(id.clone()) {
                return Vec::new();
            }
            let text = |key: &str| input.get(key).and_then(Value::as_str).map(str::to_string);
            let task = text("description").or_else(|| text("prompt")).unwrap_or_default();
            return vec![Event::SubagentStarted(Subagent { id, task, kind: text("subagent_type"), model: text("model") })];
        }
        if let Some(tool) = tools::todo_tool(&name) {
            self.hidden.insert(id.clone());
            return self.edit_todos(id, tool, &input);
        }
        let edit = tools::edit_of(&name, &input).map(|edit| Event::ToolEdit { id: id.clone(), edit });
        let mut events = if self.running.contains(&id) {
            vec![Event::ToolInput { id, file: tools::file(&input), input }]
        } else {
            self.announce(id, name, input, parent)
        };
        events.extend(edit);
        events
    }

    fn edit_todos(&mut self, id: ToolId, tool: TodoTool, input: &Value) -> Vec<Event> {
        let text = |value: &Value, key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
        match tool {
            TodoTool::Write => {
                let items = input.get("todos").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
                self.todos = items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| Todo {
                        id: (i + 1).to_string(),
                        text: text(item, "content").unwrap_or_default(),
                        status: todo_status(text(item, "status").as_deref()),
                    })
                    .collect();
                vec![Event::Todos(self.todos.clone())]
            }
            TodoTool::Create => {
                self.creating.insert(id, text(input, "subject").unwrap_or_default());
                Vec::new()
            }
            TodoTool::Update => {
                let Some(task) = text(input, "taskId") else { return Vec::new() };
                if text(input, "status").as_deref() == Some("deleted") {
                    self.todos.retain(|todo| todo.id != task);
                } else if let Some(todo) = self.todos.iter_mut().find(|todo| todo.id == task) {
                    if let Some(status) = text(input, "status") {
                        todo.status = todo_status(Some(&status));
                    }
                    if let Some(subject) = text(input, "subject") {
                        todo.text = subject;
                    }
                }
                vec![Event::Todos(self.todos.clone())]
            }
        }
    }

    fn tool_result(&mut self, id: ToolId, content: &Value, is_error: bool, detail: Option<Value>) -> Vec<Event> {
        if let Some(subject) = self.creating.remove(&id) {
            self.hidden.remove(&id);
            let task = detail.as_ref().and_then(|d| d.get("task")?.get("id")?.as_str().map(str::to_string));
            let task = task.or_else(|| task_number(&flatten(content)));
            if is_error || task.is_none() {
                return Vec::new();
            }
            self.todos.push(Todo { id: task.unwrap_or_default(), text: subject, status: TodoStatus::Pending });
            return vec![Event::Todos(self.todos.clone())];
        }
        if self.hidden.remove(&id) {
            return Vec::new();
        }
        // A background command's result only says it started. The call stays running until it ends.
        if self.background.contains(&id) && self.running.contains(&id) {
            return Vec::new();
        }
        if !self.running.remove(&id) {
            return Vec::new();
        }
        vec![Event::ToolFinished { id, output: tool_output(&flatten(content), is_error) }]
    }

    /// Ends the calls still running with an error. A background command outlives the turn that started it,
    /// so `keep_background` leaves it running; a process that ended takes it down too.
    fn fail_open_tools(&mut self, why: &str, keep_background: bool) -> Vec<Event> {
        let mut open: Vec<_> = self.running.iter().filter(|id| !(keep_background && self.background.contains(*id))).cloned().collect();
        for id in &open {
            self.running.remove(id);
        }
        open.sort();
        open.into_iter()
            .map(|id| Event::ToolFinished {
                id,
                output: ToolOutput { text: why.to_string(), is_error: true, truncated: false, full_at: None },
            })
            .collect()
    }
}
