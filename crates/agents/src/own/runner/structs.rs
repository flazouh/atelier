use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, mpsc::{Receiver, RecvTimeoutError, Sender}},
    time::{Duration, Instant},
};

use atelier_project::Project;
use serde_json::Value;

use super::super::{
    OwnOptions,
    context,
    message::{Block, Cancel, Delta, Message, Model, ModelError, ModelRequest, StopReason, ToolDef},
    permission::{self, Rules, Verdict},
    store::{self, Meta},
    tools::{self, Tool, ToolContext, ToolResult},
};
use crate::session::{
    BlockId, Choice, ChoiceId, ChoiceKind, Command, EndReason, Event, EventSink, PermissionMode,
    PermissionRequest, RequestId, Session, SessionError, SessionId, Started, ToolCall, ToolId,
    ToolOutput, ToolStatus, TurnEnd, TurnOutcome, Usage, message_text,
};
use super::types::{Answer, Ending, Inbox};
use super::helpers::sleep;

pub(super) struct Shared {
    pub(super) cancel: Cancel,
    pub(super) mode: Mutex<PermissionMode>,
    pub(super) model: Mutex<String>,
}

pub(in super::super) struct Handle {
    pub(super) tx: Sender<Inbox>,
    pub(super) shared: Arc<Shared>,
}

impl Session for Handle {
    fn send(&self, command: Command) -> Result<(), SessionError> {
        let gone = |_| SessionError::Closed;
        match command {
            Command::Send { text, attachments } => self.tx.send(Inbox::Send(message_text(&text, &attachments))).map_err(gone),
            Command::Answer { request, choice } => self.tx.send(Inbox::Answer(request, choice)).map_err(gone),
            Command::Interrupt => {
                self.shared.cancel.set();
                Ok(())
            }
            Command::SetModel { model } => {
                *self.shared.model.lock().unwrap_or_else(|p| p.into_inner()) = model;
                Ok(())
            }
            Command::SetPermissionMode { mode } => {
                *self.shared.mode.lock().unwrap_or_else(|p| p.into_inner()) = mode;
                Ok(())
            }
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.shared.cancel.set();
        let _ = self.tx.send(Inbox::Close);
    }
}

pub(super) struct Runner {
    pub(super) model: Arc<dyn Model>,
    pub(super) options: Arc<OwnOptions>,
    pub(super) project: Arc<dyn Project>,
    pub(super) sink: EventSink,
    pub(super) shared: Arc<Shared>,
    pub(super) rx: Receiver<Inbox>,
    pub(super) queue: VecDeque<String>,
    pub(super) messages: Vec<Message>,
    pub(super) meta: Meta,
    pub(super) rules: Rules,
    pub(super) next_block: u64,
    pub(super) closed: bool,
    pub(super) save_warned: bool,
}

impl Runner {
    fn emit(&self, event: Event) {
        (self.sink)(event);
    }

    fn warn(&self, text: impl Into<String>) {
        self.emit(Event::Warning(text.into()));
    }

    pub(super) fn mode(&self) -> PermissionMode {
        *self.shared.mode.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn model_id(&self) -> String {
        self.shared.model.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub(super) fn run(mut self) {
        self.emit(Event::Started(Started { session: SessionId::new(self.meta.id.clone()), model: Some(self.model_id()), mode: Some(self.mode()) , commands: Vec::new() }));
        self.meta.model = self.model_id();
        loop {
            let text = match self.queue.pop_front() {
                Some(text) => text,
                None => match self.rx.recv() {
                    Ok(Inbox::Send(text)) => text,
                    Ok(Inbox::Answer(..)) => continue,
                    Ok(Inbox::Close) | Err(_) => break,
                },
            };
            if self.closed {
                break;
            }
            self.shared.cancel.clear();
            let ending = self.turn(text);
            if self.closed {
                break;
            }
            let (outcome, summary) = match ending {
                Ending::Done(summary) => (TurnOutcome::Completed, summary),
                Ending::Interrupted => (TurnOutcome::Interrupted, None),
                Ending::Failed(why) => (TurnOutcome::Failed(why), None),
            };
            self.emit(Event::TurnEnded(TurnEnd { outcome, summary }));
        }
        self.emit(Event::Ended(EndReason::Closed));
    }

    fn persist(&mut self) {
        if self.meta.title.is_empty() {
            self.meta.title = self.messages.iter().find(|m| m.role == super::super::message::Role::User).map(|m| store::title_of(&m.text())).unwrap_or_default();
        }
        self.meta.updated = store::now();
        self.meta.model = self.model_id();
        if let Err(why) = store::save(self.project.as_ref(), &self.meta, &self.messages)
            && !self.save_warned
        {
            self.save_warned = true;
            self.warn(format!("This session is not being saved: {why}"));
        }
    }

    pub(super) fn turn(&mut self, text: String) -> Ending {
        self.messages.push(Message::user(text));
        self.persist();
        let tools = tools::all();
        let defs = tools::definitions(tools);
        let system = super::super::system_prompt(self.project.root(), &self.options.system_extra);
        for step in 0.. {
            if self.shared.cancel.is_set() {
                return Ending::Interrupted;
            }
            if step >= self.options.max_steps {
                return Ending::Failed(format!("stopped after {} steps without an answer", self.options.max_steps));
            }
            let compaction = context::compact(&mut self.messages, &self.options.context);
            if compaction.shortened > 0 {
                self.warn(format!("Shortened {} old tool results to fit the context.", compaction.shortened));
            }
            let (reply, _) = match self.call_model(&system, &defs) {
                Ok(done) => done,
                Err((ModelError::Cancelled, partial)) => {
                    if !partial.is_empty() {
                        self.messages.push(Message::assistant(partial));
                        self.persist();
                    }
                    return Ending::Interrupted;
                }
                Err((error, _)) => return Ending::Failed(error.to_string()),
            };
            self.emit(Event::Usage(Usage {
                input_tokens: reply.usage.input,
                output_tokens: reply.usage.output,
                cache_read_tokens: reply.usage.cache_read,
                cache_write_tokens: reply.usage.cache_write,
                cost_usd: None,
            }));
            let cut_off = matches!(reply.stop, StopReason::MaxTokens | StopReason::Refusal);
            // A call the reply cut off, or that a refusal ended, has no result to come; the conversation
            // must not keep it.
            let blocks: Vec<Block> = reply.blocks.iter().filter(|b| !(cut_off && matches!(b, Block::ToolUse { .. }))).cloned().collect();
            let summary = blocks.iter().rev().find_map(|b| if let Block::Text { text } = b { Some(text.clone()) } else { None });
            if !blocks.is_empty() {
                self.messages.push(Message::assistant(blocks.clone()));
                self.persist();
            }
            match reply.stop {
                StopReason::MaxTokens => return Ending::Failed("the reply was cut off by the token limit".into()),
                StopReason::Refusal => return Ending::Failed("the model declined to answer".into()),
                _ => {}
            }
            let calls: Vec<&Block> = blocks.iter().filter(|b| matches!(b, Block::ToolUse { .. })).collect();
            if calls.is_empty() {
                return Ending::Done(summary);
            }
            let (results, interrupted) = self.run_tools(&calls, &reply.malformed, tools);
            self.messages.push(Message { role: super::super::message::Role::User, blocks: results });
            self.persist();
            if interrupted {
                return Ending::Interrupted;
            }
        }
        Ending::Failed("the loop did not end".into())
    }

    /// One model call, retried while the failure is one a moment may cure and nothing was shown yet.
    #[allow(clippy::type_complexity)]
    fn call_model(&mut self, system: &str, defs: &[ToolDef]) -> Result<(super::super::message::Reply, Vec<Block>), (ModelError, Vec<Block>)> {
        let model_id = self.model_id();
        let mut attempt = 0;
        loop {
            let mut bridge = Bridge::new(self.sink.clone(), self.next_block);
            let request = ModelRequest {
                model: &model_id,
                system,
                tools: defs,
                messages: &self.messages,
                max_tokens: self.options.max_tokens,
                thinking: self.options.thinking,
            };
            let result = self.model.stream(&request, &mut |delta| bridge.on(delta), &self.shared.cancel);
            self.next_block = bridge.next_block;
            match result {
                Ok(reply) => return Ok((reply, bridge.partial())),
                Err(error) if error.is_retryable() && !bridge.shown && attempt < self.options.retry.max_retries && !self.shared.cancel.is_set() => {
                    let wait = match &error {
                        ModelError::RateLimited { retry_after: Some(after), .. } => (*after).min(self.options.retry.max_delay),
                        _ => self.options.retry.delay(attempt),
                    };
                    self.warn(format!("{error}. Trying again in {:.0} s.", wait.as_secs_f64().ceil()));
                    if !sleep(wait, &self.shared.cancel) {
                        return Err((ModelError::Cancelled, Vec::new()));
                    }
                    attempt += 1;
                }
                Err(error) => return Err((error, bridge.partial())),
            }
        }
    }

    fn run_tools(&mut self, calls: &[&Block], malformed: &[String], tools: &'static [Box<dyn Tool>]) -> (Vec<Block>, bool) {
        let mut results = Vec::new();
        let mut interrupted = false;
        for call in calls {
            let Block::ToolUse { id, name, input } = call else { continue };
            if interrupted || self.shared.cancel.is_set() {
                interrupted = true;
                self.finish(id, ToolResult::err("interrupted by the user before this ran"), &mut results);
                continue;
            }
            let result = if malformed.contains(id) {
                ToolResult::err("the input of this call was not valid JSON, so it did not run. Send the call again with valid JSON.")
            } else if let Some(tool) = tools.iter().find(|t| t.name() == name) {
                self.run_one(tool.as_ref(), id, input, &mut interrupted)
            } else {
                ToolResult::err(format!("there is no tool named {name}. The tools are: {}", tools.iter().map(|t| t.name()).collect::<Vec<_>>().join(", ")))
            };
            self.finish(id, result, &mut results);
            if self.shared.cancel.is_set() {
                interrupted = true;
            }
        }
        (results, interrupted)
    }

    fn finish(&self, id: &str, result: ToolResult, results: &mut Vec<Block>) {
        let truncated = result.text.len() > ToolOutput::MAX_TEXT;
        self.emit(Event::ToolFinished {
            id: ToolId::new(id),
            output: ToolOutput { text: tools::cap(&result.text, ToolOutput::MAX_TEXT), is_error: result.is_error, truncated, full_at: None },
        });
        results.push(Block::ToolResult { id: id.to_string(), content: result.text, is_error: result.is_error });
    }

    fn call_of(&self, tool: &dyn Tool, id: &str, input: &Value, status: ToolStatus) -> ToolCall {
        ToolCall { id: ToolId::new(id), name: tool.name().into(), kind: tool.kind(), input: input.clone(), file: tool.file(input), parent: None, status }
    }

    fn run_one(&mut self, tool: &dyn Tool, id: &str, input: &Value, interrupted: &mut bool) -> ToolResult {
        let file = tool.file(input);
        self.emit(Event::ToolInput { id: ToolId::new(id), input: input.clone(), file: file.clone() });
        if let Some(file) = file {
            self.emit(Event::ToolTarget { id: ToolId::new(id), file });
        }
        match permission::decide(self.mode(), tool.access(), tool.name(), &self.rules) {
            Verdict::Allow => {}
            Verdict::Deny(why) => return ToolResult::err(why),
            Verdict::Ask => match self.ask(tool, id, input) {
                Answer::Allow => {}
                Answer::AllowAlways => self.rules.allow_always(tool.name()),
                Answer::Deny => return ToolResult::err("The user did not allow this action. Do not try it again; ask what they want instead."),
                Answer::Interrupted => {
                    *interrupted = true;
                    return ToolResult::err("interrupted by the user before this ran");
                }
            },
        }
        self.emit(Event::ToolStatus { id: ToolId::new(id), status: ToolStatus::Running });
        let ctx = ToolContext { project: self.project.as_ref(), cancel: &self.shared.cancel };
        tool.run(&ctx, input)
    }

    fn ask(&mut self, tool: &dyn Tool, id: &str, input: &Value) -> Answer {
        let request = RequestId::new(format!("permission-{id}"));
        let choice = |id: &str, label: &str, kind| Choice { id: ChoiceId::new(id), label: label.into(), kind };
        self.emit(Event::Permission(PermissionRequest {
            id: request.clone(),
            call: self.call_of(tool, id, input, ToolStatus::Pending),
            reason: None,
            choices: vec![
                choice("allow", "Allow", ChoiceKind::Allow),
                choice("allow-always", "Always allow", ChoiceKind::AllowAlways),
                choice("deny", "Deny", ChoiceKind::Deny),
            ],
        }));
        loop {
            if self.shared.cancel.is_set() || self.closed {
                self.emit(Event::PermissionCancelled(request));
                return Answer::Interrupted;
            }
            match self.rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Inbox::Answer(answered, choice)) if answered == request => {
                    return match choice.as_str() {
                        "allow" => Answer::Allow,
                        "allow-always" => Answer::AllowAlways,
                        _ => Answer::Deny,
                    };
                }
                Ok(Inbox::Answer(..)) => {}
                Ok(Inbox::Send(text)) => self.queue.push_back(text),
                Ok(Inbox::Close) | Err(RecvTimeoutError::Disconnected) => {
                    self.closed = true;
                    self.shared.cancel.set();
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }
}

/// Turns the model's deltas into events, and keeps what streamed so an interrupt can keep it.
struct Bridge {
    pub(super) sink: EventSink,
    pub(super) next_block: u64,
    /// Any event has gone out. A retry after that would show the reply twice.
    shown: bool,
    pub(super) open: Option<(BlockId, bool, Instant)>,
    text: String,
    finished: Vec<Block>,
}

impl Bridge {
    pub(super) fn new(sink: EventSink, next_block: u64) -> Self {
        Self { sink, next_block, shown: false, open: None, text: String::new(), finished: Vec::new() }
    }

    fn on(&mut self, delta: Delta) {
        match delta {
            Delta::Text(piece) => {
                let id = self.block(false);
                self.text.push_str(&piece);
                self.shown = true;
                (self.sink)(Event::Text { block: id, delta: piece });
            }
            Delta::Thinking(piece) => {
                let id = self.block(true);
                self.shown = true;
                (self.sink)(Event::Thinking { block: id, delta: piece });
            }
            Delta::BlockEnd => self.end(),
            Delta::ToolStart { id, name } => {
                self.end();
                self.shown = true;
                let (kind, _) = tools::describe(&name, &Value::Null);
                (self.sink)(Event::ToolStarted(ToolCall { id: ToolId::new(id), name, kind, input: Value::Null, file: None, parent: None, status: ToolStatus::Pending }));
            }
            Delta::ToolDone { .. } => {}
        }
    }

    fn block(&mut self, thinking: bool) -> BlockId {
        if let Some((id, is_thinking, _)) = self.open
            && is_thinking == thinking
        {
            return id;
        }
        self.end();
        let id = BlockId(self.next_block);
        self.next_block += 1;
        self.open = Some((id, thinking, Instant::now()));
        id
    }

    pub(super) fn end(&mut self) {
        if let Some((id, thinking, started)) = self.open.take() {
            if thinking {
                (self.sink)(Event::ThinkingDone { block: id, took: started.elapsed() });
            } else if !self.text.is_empty() {
                self.finished.push(Block::Text { text: std::mem::take(&mut self.text) });
            }
        }
    }

    /// The text that streamed, for an interrupted reply.
    fn partial(&mut self) -> Vec<Block> {
        self.end();
        std::mem::take(&mut self.finished)
    }
}
