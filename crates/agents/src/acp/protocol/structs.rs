use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Instant},
};

use serde_json::Value;

use super::super::{
    AcpAgent,
    client::{self, Outgoing},
    map::Mapper,
    rpc::{self, Incoming, RpcError},
    wire::{self, SessionUpdate},
};
use crate::{
    session::{Command, EndReason, Event, OpenRequest, RequestId, SessionError, SessionId, Started, TurnEnd, TurnOutcome, Usage, message_text},
    subprocess,
};
use super::types::{AUTH_REQUIRED, Asked, Found, Goal, Phase, SETTINGS_WAIT};
use super::helpers::{model_name, parse, stop_outcome, summaries, usage, warning};

/// What one input gives: events for the app, lines for the agent, and whether the connection is over.
#[derive(Debug, Default)]
pub(in super::super) struct Step {
    pub events: Vec<Event>,
    pub lines: Vec<String>,
    pub done: bool,
}

pub(in super::super) struct Protocol {
    pub(super) agent: Arc<AcpAgent>,
    pub(super) cwd: String,
    pub(super) goal: Goal,
    pub(super) phase: Phase,
    pub(super) next_id: u64,
    pub(super) waiting: HashMap<u64, Asked>,
    /// When each waiting mode or model request was written, by id: an agent that never answers one does not
    /// hold the user's messages for good.
    pub(super) settings_at: HashMap<u64, Instant>,
    /// The time of the last line or command, which a request written now is stamped with.
    pub(super) now: Instant,
    /// The sign-in methods the agent offered, and whether atelier has used one.
    pub(super) auth_methods: Vec<String>,
    pub(super) authenticated: bool,
    pub(super) can_load: bool,
    pub(super) can_list: bool,
    pub(super) session: Option<SessionId>,
    /// The agent's mode and model in force (the model by atelier's name for it), the config option that
    /// sets the model when it has one, and the model ids the agent listed.
    pub(super) mode: Option<String>,
    pub(super) model: Option<String>,
    pub(super) model_option: Option<String>,
    pub(super) models: Vec<String>,
    pub(super) commands: Vec<String>,
    /// The model and mode to set once the session is ready.
    pub(super) want_model: Option<String>,
    pub(super) want_mode: Option<String>,
    /// Messages to prompt, in order: sent before the session was ready, or while a turn ran.
    pub(super) queued: VecDeque<String>,
    pub(super) in_turn: bool,
    /// The agent's permission requests atelier has not answered, with their JSON-RPC ids.
    pub(super) asked: HashMap<RequestId, Value>,
    pub(super) mapper: Mapper,
    pub(super) history: Vec<Event>,
    pub(super) found: Option<Result<Found, SessionError>>,
}

impl Protocol {
    /// A connection in `cwd` (the project's root on its host) for `goal`, and the first line to write.
    pub fn new(agent: Arc<AcpAgent>, cwd: String, goal: Goal) -> (Self, Vec<String>) {
        let (want_model, want_mode) = match &goal {
            Goal::Open(request) => (request.model.clone(), request.mode.and_then(|m| agent.mode_id(m)).map(str::to_string)),
            _ => (None, None),
        };
        let mut protocol = Self {
            agent,
            cwd,
            goal,
            phase: Phase::Starting,
            next_id: 0,
            waiting: HashMap::new(),
            settings_at: HashMap::new(),
            now: Instant::now(),
            auth_methods: Vec::new(),
            authenticated: false,
            can_load: false,
            can_list: false,
            session: None,
            mode: None,
            model: None,
            model_option: None,
            models: Vec::new(),
            commands: Vec::new(),
            want_model,
            want_mode,
            queued: VecDeque::new(),
            in_turn: false,
            asked: HashMap::new(),
            mapper: Mapper::default(),
            history: Vec::new(),
            found: None,
        };
        let first = protocol.request(Asked::Initialize, client::initialize());
        (protocol, vec![first])
    }

    /// What a connection for a list or a history found, once it is over.
    pub fn found(&mut self) -> Option<Result<Found, SessionError>> {
        self.found.take()
    }

    /// Reads one line of the agent's stdout. A line that is not JSON-RPC gives a warning.
    pub fn line(&mut self, line: &str, now: Instant) -> Step {
        self.now = now;
        let line = line.trim();
        if line.is_empty() || self.phase == Phase::Over {
            return Step::default();
        }
        match rpc::parse(line) {
            Ok(Incoming::Response { id, outcome }) => self.response(&id, outcome, now),
            Ok(Incoming::Request { id, method, params }) => self.request_from_agent(id, &method, params, now),
            Ok(Incoming::Notification { method, params }) if method == "session/update" => self.update(params, now),
            Ok(Incoming::Notification { .. }) => Step::default(),
            Err(error) => {
                let mut step = warning(format!("a line from the agent did not parse: {error}"));
                self.prompt_next(&mut step);
                step
            }
        }
    }

    /// Takes one of the app's commands. It never waits for the agent: a message sent before the session
    /// is ready, or while a turn runs, is prompted in its turn.
    pub fn command(&mut self, command: Command, now: Instant) -> Result<Step, SessionError> {
        self.now = now;
        if self.phase == Phase::Over {
            return Err(SessionError::Closed);
        }
        let mut step = Step::default();
        match command {
            Command::Send { text, attachments } => {
                self.queued.push_back(message_text(&text, &attachments));
                self.prompt_next(&mut step);
            }
            Command::Answer { request, choice } => {
                let id = self.asked.remove(&request).ok_or(SessionError::Unsupported("an answer to a request that is not waiting"))?;
                step.lines.push(rpc::result(&id, client::permission_selected(choice.as_str())));
            }
            Command::Interrupt => self.interrupt(&mut step),
            Command::SetModel { model } => match self.ready_session() {
                Some(session) => step.lines.push(self.set_model(&session, model)),
                None => self.want_model = Some(model),
            },
            Command::SetPermissionMode { mode } => {
                let id = self.agent.mode_id(mode).ok_or(SessionError::Unsupported("this permission mode"))?.to_string();
                match self.ready_session() {
                    Some(session) => step.lines.push(self.request(Asked::SetMode(id.clone()), client::set_mode(session.as_str(), &id))),
                    None => self.want_mode = Some(id),
                }
            }
        }
        Ok(step)
    }

    /// The events for an agent whose process ended: `code` is its exit code, `None` for a signal. A turn
    /// or a call still open fails and a waiting question is withdrawn, so the UI never waits for an agent
    /// that is gone. A connection ends once: a second call to `exited` or `closed` gives nothing.
    pub fn exited(&mut self, code: Option<i32>, stderr: &str, now: Instant) -> Vec<Event> {
        if std::mem::replace(&mut self.phase, Phase::Over) == Phase::Over {
            return Vec::new();
        }
        let tail = subprocess::stderr_tail(stderr);
        let why = subprocess::exit_why(code, &tail);
        if !matches!(self.goal, Goal::Open(_)) {
            self.found.get_or_insert(Err(SessionError::Read(why)));
            return Vec::new();
        }
        let mut events = self.cut_short(&why, now);
        events.push(Event::Ended(EndReason::Exited { code, stderr: tail }));
        events
    }

    /// The event for a session atelier closed on purpose: it ends, and nothing else fails.
    pub fn closed(&mut self) -> Vec<Event> {
        if std::mem::replace(&mut self.phase, Phase::Over) == Phase::Over { Vec::new() } else { vec![Event::Ended(EndReason::Closed)] }
    }

    pub(super) fn response(&mut self, id: &Value, outcome: Result<Value, RpcError>, now: Instant) -> Step {
        // An id atelier wrote as a number may come back as text.
        let number = id.as_u64().or_else(|| id.as_str()?.parse().ok());
        let Some(asked) = number.and_then(|id| {
            self.settings_at.remove(&id);
            self.waiting.remove(&id)
        }) else {
            return Step::default();
        };
        let mut step = Step::default();
        match (asked, outcome) {
            (Asked::Initialize, Ok(result)) => {
                let initialized: wire::Initialized = parse(result).unwrap_or_default();
                self.auth_methods = initialized.auth_methods.into_iter().map(|m| m.id).collect();
                self.can_load = initialized.agent_capabilities.load_session;
                self.can_list = initialized.agent_capabilities.session_capabilities.list.is_some();
                self.open(&mut step, now);
            }
            (Asked::Authenticate, Ok(_)) => self.open(&mut step, now),
            (Asked::Open | Asked::List, Err(error)) if self.can_sign_in(&error) => {
                self.authenticated = true;
                let method = self.auth_methods[0].clone();
                step.lines.push(self.request(Asked::Authenticate, client::authenticate(&method)));
            }
            (Asked::Initialize | Asked::Authenticate | Asked::Open, Err(error)) => self.fail(&mut step, error.to_string(), now),
            (Asked::Open, Ok(result)) => self.opened(&mut step, parse(result).unwrap_or_default(), now),
            (Asked::List, Ok(result)) => {
                let found = parse::<wire::Listed>(result).map(|listed| Found::Sessions(summaries(listed)));
                self.finish(&mut step, found.map_err(SessionError::Read));
            }
            (Asked::List, Err(error)) => self.finish(&mut step, Err(SessionError::Read(error.to_string()))),
            (Asked::Prompt, outcome) => {
                let (outcome, usage) = match outcome.map(parse::<wire::Prompted>) {
                    Ok(Ok(prompted)) => (stop_outcome(&prompted.stop_reason), prompted.usage.map(usage)),
                    Ok(Err(error)) => (TurnOutcome::Failed(format!("the agent's answer did not parse: {error}")), None),
                    Err(error) => (TurnOutcome::Failed(error.to_string()), None),
                };
                self.end_turn(&mut step, outcome, usage, now);
                self.prompt_next(&mut step);
            }
            (Asked::SetMode(mode), Ok(_)) => self.set(&mut step, |p| p.mode = Some(mode)),
            (Asked::SetModel(model), Ok(_)) => self.set(&mut step, |p| p.model = Some(model)),
            (Asked::SetMode(_), Err(error)) => step.events.push(Event::Warning(format!("the agent kept its mode: {error}"))),
            (Asked::SetModel(_), Err(error)) => step.events.push(Event::Warning(format!("the agent kept its model: {error}"))),
        }
        self.prompt_next(&mut step);
        step
    }

    pub(super) fn can_sign_in(&self, error: &RpcError) -> bool {
        error.code == AUTH_REQUIRED && !self.authenticated && !self.auth_methods.is_empty()
    }

    /// Asks for the session the goal needs, once the agent is initialized (and signed in, if it asked).
    pub(super) fn open(&mut self, step: &mut Step, now: Instant) {
        let outgoing = match self.goal.clone() {
            Goal::Open(OpenRequest { resume: None, .. }) => client::new_session(&self.cwd),
            Goal::Open(OpenRequest { resume: Some(session), .. }) | Goal::History(session) if self.can_load => {
                client::load_session(&self.cwd, session.as_str())
            }
            Goal::Open(_) | Goal::History(_) => return self.fail(step, "this agent cannot resume a session".into(), now),
            Goal::List if self.can_list => {
                step.lines.push(self.request(Asked::List, client::list_sessions(&self.cwd)));
                return;
            }
            Goal::List => return self.finish(step, Err(SessionError::Unsupported("a session list"))),
        };
        step.lines.push(self.request(Asked::Open, outgoing));
    }

    pub(super) fn opened(&mut self, step: &mut Step, opened: wire::Opened, now: Instant) {
        let session = match (&self.goal, opened.session_id) {
            (_, Some(id)) => SessionId::new(id),
            (Goal::Open(OpenRequest { resume: Some(id), .. }) | Goal::History(id), None) => id.clone(),
            _ => return self.fail(step, "the agent started a session with no id".into(), now),
        };
        if let Goal::History(_) = self.goal {
            let mut history = std::mem::take(&mut self.history);
            history.extend(self.mapper.end_of_history(now));
            return self.finish(step, Ok(Found::History(history)));
        }
        self.mode = opened.modes.map(|m| m.current_mode_id);
        let model_option = opened.config_options.iter().find(|o| o.is("model"));
        self.model_option = model_option.map(|o| o.id.clone());
        let listed = model_option.and_then(|o| o.current_value.as_ref()?.as_str().map(str::to_string));
        if let Some(models) = opened.models {
            self.models = models.available_models.into_iter().map(|m| m.model_id).collect();
            self.model = Some(model_name(listed.as_deref().unwrap_or(&models.current_model_id)));
        } else {
            self.model = listed.as_deref().map(model_name);
        }
        self.session = Some(session.clone());
        self.phase = Phase::Ready;
        step.events.push(self.started());
        if let Some(model) = self.want_model.take().filter(|m| self.model.as_deref() != Some(&model_name(m))) {
            step.lines.push(self.set_model(&session, model));
        }
        if let Some(mode) = self.want_mode.take().filter(|m| self.mode.as_ref() != Some(m)) {
            step.lines.push(self.request(Asked::SetMode(mode.clone()), client::set_mode(session.as_str(), &mode)));
        }
        self.prompt_next(step);
    }

    pub(super) fn update(&mut self, params: Value, now: Instant) -> Step {
        let update = match parse::<wire::Notification>(params) {
            Ok(notification) => notification.update,
            Err(error) => return warning(format!("an update from the agent did not parse: {error}")),
        };
        let mut step = Step::default();
        match update {
            SessionUpdate::AvailableCommandsUpdate(commands) => {
                let names = commands.available_commands.into_iter().map(|c| c.name).collect();
                self.set(&mut step, |p| p.commands = names);
            }
            SessionUpdate::CurrentModeUpdate(modes) => self.set(&mut step, |p| p.mode = Some(modes.current_mode_id)),
            SessionUpdate::ConfigOptionUpdate(options) => {
                let model = options.config_options.into_iter().find(|o| o.is("model"));
                if let Some(value) = model.as_ref().and_then(|o| o.current_value.as_ref()?.as_str()) {
                    let value = model_name(value);
                    self.set(&mut step, |p| p.model = Some(value));
                }
            }
            update => match (&self.goal, self.phase) {
                (Goal::History(_), _) => self.history.extend(self.mapper.update(update, now)),
                // A live session resumes from the history the app read; what `session/load` replays is that.
                (Goal::Open(_), Phase::Ready) => step.events = self.mapper.update(update, now),
                _ => {}
            },
        }
        step
    }

    pub(super) fn request_from_agent(&mut self, id: Value, method: &str, params: Value, now: Instant) -> Step {
        let mut step = Step::default();
        if method != "session/request_permission" {
            step.lines.push(rpc::error(&id, rpc::METHOD_NOT_FOUND, &format!("atelier does not serve {method}")));
            return step;
        }
        match parse::<wire::PermissionAsked>(params) {
            Ok(asked) if self.in_turn => {
                let request = RequestId::new(match &id {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                });
                self.asked.insert(request.clone(), id);
                step.events = self.mapper.permission(request, asked, now);
            }
            // A question with no turn to belong to, or one atelier cannot read, is declined.
            Ok(_) => step.lines.push(rpc::result(&id, client::permission_cancelled())),
            Err(error) => {
                step.lines.push(rpc::result(&id, client::permission_cancelled()));
                step.events.push(Event::Warning(format!("a question from the agent did not parse: {error}")));
            }
        }
        step
    }

    pub(super) fn interrupt(&mut self, step: &mut Step) {
        if !self.in_turn {
            // Nothing runs: the messages that wait for the session are dropped, and their turn ends.
            if !std::mem::take(&mut self.queued).is_empty() {
                step.events.push(Event::TurnEnded(TurnEnd { outcome: TurnOutcome::Interrupted, summary: None }));
            }
            return;
        }
        let Some(session) = self.session.clone() else { return };
        let cancel = client::cancel(session.as_str());
        step.lines.push(rpc::notification(cancel.method, cancel.params));
        // ACP asks the client to answer each waiting question as cancelled; the prompt's answer then ends the turn.
        let mut asked: Vec<_> = self.asked.drain().collect();
        asked.sort_by(|a, b| a.0.cmp(&b.0));
        for (request, id) in asked {
            step.lines.push(rpc::result(&id, client::permission_cancelled()));
            step.events.push(Event::PermissionCancelled(request));
        }
    }

    /// Prompts the next message, unless a turn runs or a model or mode atelier asked for is not answered yet:
    /// a turn runs on the settings the user chose.
    pub(super) fn prompt_next(&mut self, step: &mut Step) {
        if self.in_turn || self.settings_open() {
            return;
        }
        let Some(session) = self.ready_session() else { return };
        let Some(text) = self.queued.pop_front() else { return };
        self.in_turn = true;
        step.lines.push(self.request(Asked::Prompt, client::prompt(session.as_str(), &text)));
    }

    /// Whether a mode or model request is waiting for its answer and is not older than [`SETTINGS_WAIT`].
    pub(super) fn settings_open(&self) -> bool {
        self.settings_at.values().any(|at| self.now.saturating_duration_since(*at) < SETTINGS_WAIT)
    }

    pub(super) fn end_turn(&mut self, step: &mut Step, outcome: TurnOutcome, usage: Option<Usage>, now: Instant) {
        self.in_turn = false;
        let mut asked: Vec<_> = self.asked.drain().map(|(request, _)| request).collect();
        asked.sort();
        step.events.extend(asked.into_iter().map(Event::PermissionCancelled));
        step.events.extend(self.mapper.turn_ended(outcome, usage, now));
    }

    /// Ends a connection that cannot go on.
    pub(super) fn fail(&mut self, step: &mut Step, why: String, now: Instant) {
        if !matches!(self.goal, Goal::Open(_)) {
            return self.finish(step, Err(SessionError::Start(why)));
        }
        self.phase = Phase::Over;
        step.events.extend(self.cut_short(&why, now));
        step.events.push(Event::Ended(EndReason::Failed(why)));
        step.done = true;
    }

    /// The events that close what a gone agent left open: its calls, its questions, its turn.
    pub(super) fn cut_short(&mut self, why: &str, now: Instant) -> Vec<Event> {
        let mut events = self.mapper.gone(why, now);
        let mut asked: Vec<_> = self.asked.drain().map(|(request, _)| request).collect();
        asked.sort();
        events.extend(asked.into_iter().map(Event::PermissionCancelled));
        let waiting = !std::mem::take(&mut self.queued).is_empty();
        if std::mem::take(&mut self.in_turn) || waiting {
            events.push(Event::TurnEnded(TurnEnd { outcome: TurnOutcome::Failed(why.to_string()), summary: None }));
        }
        events
    }

    pub(super) fn finish(&mut self, step: &mut Step, found: Result<Found, SessionError>) {
        self.found = Some(found);
        self.phase = Phase::Over;
        step.done = true;
    }

    /// Changes what `Started` says, and says it again when the session has started and it changed.
    pub(super) fn set(&mut self, step: &mut Step, change: impl FnOnce(&mut Self)) {
        let before = (self.mode.clone(), self.model.clone(), self.commands.clone());
        change(self);
        if self.phase == Phase::Ready && before != (self.mode.clone(), self.model.clone(), self.commands.clone()) {
            step.events.push(self.started());
        }
    }

    pub(super) fn started(&self) -> Event {
        Event::Started(Started {
            session: self.session.clone().unwrap_or_else(|| SessionId::new("")),
            model: self.model.clone(),
            mode: self.mode.as_deref().and_then(|id| self.agent.mode_for(id)),
            commands: self.commands.clone(),
        })
    }

    pub(super) fn ready_session(&self) -> Option<SessionId> {
        (self.phase == Phase::Ready).then(|| self.session.clone()).flatten()
    }

    /// Sets `model`, by atelier's name or the agent's whole id. The request remembers atelier's name, which
    /// `Started` then says.
    pub(super) fn set_model(&mut self, session: &SessionId, model: String) -> String {
        let id = self.models.iter().find(|id| **id == model || model_name(id) == model).cloned().unwrap_or_else(|| model.clone());
        let outgoing = match &self.model_option {
            Some(option) => client::set_config_option(session.as_str(), option, &id),
            None => client::set_model(session.as_str(), &id),
        };
        self.request(Asked::SetModel(model_name(&model)), outgoing)
    }

    pub(super) fn request(&mut self, asked: Asked, outgoing: Outgoing) -> String {
        let id = self.next_id;
        self.next_id += 1;
        if matches!(asked, Asked::SetMode(_) | Asked::SetModel(_)) {
            self.settings_at.insert(id, self.now);
        }
        self.waiting.insert(id, asked);
        rpc::request(id, outgoing.method, outgoing.params)
    }
}
