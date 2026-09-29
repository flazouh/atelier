//! The loop. One thread per session owns the conversation. A message starts a turn: call the model, show
//! its reply as it streams, run the tools it asks for (asking the reader first when the mode says so),
//! call the model again with the results, and stop when it stops asking. `Interrupt` sets a flag that the
//! model stream, the permission wait and the running tool all look at.
use std::{
    collections::VecDeque,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, RecvTimeoutError, Sender, channel},
    },
    thread,
    time::{Duration, Instant},
};

use lathe_project::Project;
use serde_json::Value;

use super::{
    OwnOptions,
    context,
    message::{Block, Cancel, Delta, Message, Model, ModelError, ModelRequest, StopReason, ToolDef},
    permission::{self, Rules, Verdict},
    store::{self, Meta},
    tools::{self, Tool, ToolContext, ToolResult},
};
use crate::session::{
    BlockId, Choice, ChoiceId, ChoiceKind, Command, EndReason, Event, EventSink, PermissionMode, PermissionRequest, RequestId, Session,
    SessionError, SessionId, Started, ToolCall, ToolId, ToolOutput, ToolStatus, TurnEnd, TurnOutcome, Usage, message_text,
};

/// What the session handle tells the thread.
enum Inbox {
    Send(String),
    Answer(RequestId, ChoiceId),
    Close,
}

struct Shared {
    cancel: Cancel,
    mode: Mutex<PermissionMode>,
    model: Mutex<String>,
}

pub(super) struct Handle {
    tx: Sender<Inbox>,
    shared: Arc<Shared>,
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

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn new_id() -> String {
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    format!("own-{millis:x}-{:x}", COUNTER.fetch_add(1, Ordering::SeqCst))
}

pub(super) fn open(
    model: Arc<dyn Model>,
    options: Arc<OwnOptions>,
    project: Arc<dyn Project>,
    request: crate::session::OpenRequest,
    sink: EventSink,
) -> Result<Box<dyn Session>, SessionError> {
    let (meta, messages) = match &request.resume {
        Some(id) => {
            let (meta, messages) = store::load(project.as_ref(), id.as_str())?;
            (meta, messages)
        }
        None => (Meta { id: new_id(), ..Meta::default() }, Vec::new()),
    };
    let chosen = request.model.clone().or_else(|| (!meta.model.is_empty()).then(|| meta.model.clone())).unwrap_or_else(|| options.default_model.clone());
    let mode = request.mode.unwrap_or(PermissionMode::Ask);
    let shared = Arc::new(Shared { cancel: Cancel::default(), mode: Mutex::new(mode), model: Mutex::new(chosen) });
    let (tx, rx) = channel();
    let runner = Runner {
        rules: Rules::new(options.allow.clone()),
        model,
        options,
        project,
        sink,
        shared: shared.clone(),
        rx,
        queue: VecDeque::new(),
        messages,
        meta,
        next_block: 0,
        closed: false,
        save_warned: false,
    };
    thread::Builder::new()
        .name("lathe-own-agent".into())
        .spawn(move || runner.run())
        .map_err(|e| SessionError::Start(e.to_string()))?;
    Ok(Box::new(Handle { tx, shared }))
}

struct Runner {
    model: Arc<dyn Model>,
    options: Arc<OwnOptions>,
    project: Arc<dyn Project>,
    sink: EventSink,
    shared: Arc<Shared>,
    rx: Receiver<Inbox>,
    queue: VecDeque<String>,
    messages: Vec<Message>,
    meta: Meta,
    rules: Rules,
    next_block: u64,
    closed: bool,
    save_warned: bool,
}

/// How a turn ended, before it is told to the UI.
enum Ending {
    Done(Option<String>),
    Interrupted,
    Failed(String),
}

impl Runner {
    fn emit(&self, event: Event) {
        (self.sink)(event);
    }

    fn warn(&self, text: impl Into<String>) {
        self.emit(Event::Warning(text.into()));
    }

    fn mode(&self) -> PermissionMode {
        *self.shared.mode.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn model_id(&self) -> String {
        self.shared.model.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn run(mut self) {
        self.emit(Event::Started(Started { session: SessionId::new(self.meta.id.clone()), model: Some(self.model_id()), mode: Some(self.mode()) }));
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
            self.meta.title = self.messages.iter().find(|m| m.role == super::message::Role::User).map(|m| store::title_of(&m.text())).unwrap_or_default();
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

    fn turn(&mut self, text: String) -> Ending {
        self.messages.push(Message::user(text));
        self.persist();
        let tools = tools::all();
        let defs = tools::definitions(tools);
        let system = super::system_prompt(self.project.root(), &self.options.system_extra);
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
            self.messages.push(Message { role: super::message::Role::User, blocks: results });
            self.persist();
            if interrupted {
                return Ending::Interrupted;
            }
        }
        Ending::Failed("the loop did not end".into())
    }

    /// One model call, retried while the failure is one a moment may cure and nothing was shown yet.
    #[allow(clippy::type_complexity)]
    fn call_model(&mut self, system: &str, defs: &[ToolDef]) -> Result<(super::message::Reply, Vec<Block>), (ModelError, Vec<Block>)> {
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

enum Answer {
    Allow,
    AllowAlways,
    Deny,
    Interrupted,
}

/// Sleeps `total`, in slices, and stops early when `cancel` is set. `false` when it was.
fn sleep(total: Duration, cancel: &Cancel) -> bool {
    let end = Instant::now() + total;
    while Instant::now() < end {
        if cancel.is_set() {
            return false;
        }
        thread::sleep(Duration::from_millis(20).min(end.saturating_duration_since(Instant::now())));
    }
    !cancel.is_set()
}

/// Turns the model's deltas into events, and keeps what streamed so an interrupt can keep it.
struct Bridge {
    sink: EventSink,
    next_block: u64,
    /// Any event has gone out. A retry after that would show the reply twice.
    shown: bool,
    open: Option<(BlockId, bool, Instant)>,
    text: String,
    finished: Vec<Block>,
}

impl Bridge {
    fn new(sink: EventSink, next_block: u64) -> Self {
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

    fn end(&mut self) {
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

pub(super) fn root_name(root: &Path) -> String {
    root.file_name().and_then(|n| n.to_str()).unwrap_or("the project").to_string()
}
