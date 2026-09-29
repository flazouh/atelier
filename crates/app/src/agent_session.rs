//! One agent session in a project: the agent's own session, the queue its events land in, the
//! `Conversation` they fold into, and the list that draws it.
//!
//! Events arrive on the agent's threads. The queue joins a block's deltas and wakes this entity once
//! when it goes from empty to not; the entity then drains everything waiting, folds it, and asks for
//! one repaint, so a fast stream costs a repaint a frame. Only the rows whose content changed are
//! measured again (`list_diff`), so the list keeps its scroll while text streams.

use std::{sync::Arc, time::SystemTime};

use beui::session_status::SessionStatus;
use futures_channel::mpsc;
use futures_util::StreamExt;
use std::{collections::HashMap, time::Instant};

use beui::{PromptInput, PromptInputEvent, PromptModel};
use gpui_kit::{AppContext, Context, Entity, EventEmitter, ListAlignment, ListState, SharedString, Subscription, Task, Window, px};
use lathe_agents::{
    registry::Agent,
    session::{
        Answer, ChoiceKind, Command, Conversation, Event, EventQueue, Item, OpenRequest, PermissionMode, Session,
        SessionError, SessionId,
    },
};
use lathe_project::Project;

use crate::{list_diff, status};

/// What the session tells the shell.
pub enum SessionEvent {
    /// Its title, status or id changed: the sidebar and the tabs draw it again.
    Changed,
    /// The reader named it: the name is kept across launches.
    Renamed,
}

impl EventEmitter<SessionEvent> for AgentSession {}

/// Seconds since the Unix epoch, for "2m ago".
pub fn now() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

pub struct AgentSession {
    /// The panel's id: stable from the start, before the agent names the session.
    pub key: SharedString,
    pub agent: Agent,
    project: Arc<dyn Project>,
    session: Option<Box<dyn Session>>,
    queue: EventQueue,
    pub conversation: Conversation,
    /// The agent's id for the session, once it says it.
    pub id: Option<SessionId>,
    pub title: SharedString,
    /// A name the reader gave it, over the first message.
    pub name: Option<SharedString>,
    pub status: SessionStatus,
    pub active_at: u64,
    /// The reader is looking at it: a turn that ends is seen.
    pub seen: bool,
    pub model: Option<String>,
    pub mode: Option<PermissionMode>,
    /// Why it could not start, or a message that did not go.
    pub problem: Option<SharedString>,
    /// What the agent wrote to stderr as it failed: the tail behind "Show details".
    pub stderr: Option<SharedString>,
    /// Starting, or reading its history: the panel shows it.
    pub starting: bool,
    /// A message written while the agent was not running: it goes once the session resumes.
    waiting_send: Option<String>,
    pub list: ListState,
    rows: Vec<(u8, usize, usize)>,
    pub composer: Entity<PromptInput>,
    /// The name being typed, while the reader renames the session.
    pub renaming: Option<Entity<gpui_kit::component::input::InputState>>,
    _renaming: Option<Subscription>,
    /// When each thinking block began, for its live "Thinking for 12s".
    pub thinking_since: HashMap<lathe_agents::session::BlockId, Instant>,
    _composer: Subscription,
    _pump: Task<()>,
    _start: Task<()>,
}

impl AgentSession {
    /// Starts a new session, or resumes `resume` after reading its history, in `project`.
    pub fn start(
        key: SharedString,
        agent: Agent,
        project: Arc<dyn Project>,
        resume: Option<(SessionId, SharedString)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (wake, mut woken) = mpsc::unbounded::<()>();
        let queue = EventQueue::new(move || drop(wake.unbounded_send(())));
        let _pump = cx.spawn_in(window, async move |this, cx| {
            while woken.next().await.is_some() {
                if this.update(cx, |s, cx| s.drain(cx)).is_err() {
                    return;
                }
            }
        });
        let models: Vec<PromptModel> = agent
            .backend
            .capabilities()
            .models
            .iter()
            .map(|m| {
                let model = PromptModel::new(m.id.clone(), m.label.clone());
                match lathe_agents::registry::model_mark(&m.id) {
                    Some(mark) => model.mark(mark),
                    None => model,
                }
            })
            .collect();
        let modes: Vec<SharedString> = agent.backend.capabilities().permission_modes.into_iter().map(|m| mode_word(m).into()).collect();
        let composer = cx.new(|cx| PromptInput::new(format!("Ask {}", agent.name), "", window, cx).models(models).modes(modes));
        let _composer = cx.subscribe(&composer, |this, _, event: &PromptInputEvent, cx| match event {
            PromptInputEvent::Submit(text) => this.send(text.to_string(), cx),
            PromptInputEvent::Stop => this.interrupt(cx),
            PromptInputEvent::ModelChanged(model) => this.set_model(model.to_string(), cx),
            PromptInputEvent::ModeChanged(mode) => {
                if let Some(mode) = mode_from(mode) {
                    this.set_mode(mode, cx)
                }
            }
            PromptInputEvent::Action(_) => {}
        });
        let (id, title) = match &resume {
            Some((id, title)) => (Some(id.clone()), title.clone()),
            None => (None, "New session".into()),
        };
        let mut this = Self {
            key,
            agent,
            project,
            session: None,
            queue,
            conversation: Conversation::new(),
            id,
            title,
            name: None,
            status: SessionStatus::Idle,
            active_at: now(),
            seen: false,
            model: None,
            mode: None,
            problem: None,
            stderr: None,
            starting: true,
            waiting_send: None,
            list: ListState::new(0, ListAlignment::Bottom, px(600.)),
            rows: Vec::new(),
            composer,
            renaming: None,
            _renaming: None,
            thinking_since: HashMap::new(),
            _composer,
            _pump,
            _start: Task::ready(()),
        };
        this.open(resume.map(|(id, _)| id), true, cx);
        this
    }

    /// Opens the agent's session off the UI thread: its history first when it resumes and the panel
    /// has not got it (`read_history`), then the agent.
    fn open(&mut self, resume: Option<SessionId>, read_history: bool, cx: &mut Context<Self>) {
        let (backend, project, sink) = (self.agent.backend.clone(), self.project.clone(), self.queue.sink());
        let request = OpenRequest { resume: resume.clone(), model: self.model.clone(), mode: self.mode };
        let opening = cx.background_spawn(async move {
            let history = match &resume {
                Some(id) if read_history => backend.history(project.as_ref(), id).unwrap_or_default(),
                _ => Vec::new(),
            };
            (history, backend.open(project, request, sink))
        });
        self._start = cx.spawn(async move |this, cx| {
            let (history, opened) = opening.await;
            _ = this.update(cx, |s, cx| {
                s.starting = false;
                for event in &history {
                    s.conversation.apply(event);
                }
                match opened {
                    Ok(session) => {
                        s.session = Some(session);
                        if let Some(text) = s.waiting_send.take() {
                            s.command(Command::send(text), cx);
                        }
                    }
                    Err(error) => {
                        s.waiting_send = None;
                        s.status = SessionStatus::Failed(beui::session_status::short_reason(&problem_words(&error)));
                        s.problem = Some(problem_words(&error).into());
                    }
                }
                s.refresh_rows();
                cx.emit(SessionEvent::Changed);
                cx.notify();
            });
        });
    }

    /// Folds every event waiting: one frame's worth.
    fn drain(&mut self, cx: &mut Context<Self>) {
        let events = self.queue.drain();
        if events.is_empty() {
            return;
        }
        let before = self.status.clone();
        for event in &events {
            if let Event::Thinking { block, .. } = event {
                self.thinking_since.entry(*block).or_insert_with(Instant::now);
            }
            self.conversation.apply(event);
            self.status = status::after(&self.status, event, self.seen);
            match event {
                Event::Started(started) => {
                    self.id = Some(started.session.clone());
                    self.model = started.model.clone().or(self.model.take());
                    self.mode = started.mode.or(self.mode);
                }
                Event::Ended(end) => {
                    self.session = None;
                    if let lathe_agents::session::EndReason::Exited { stderr, .. } = end
                        && !stderr.trim().is_empty()
                        && matches!(self.status, SessionStatus::Failed(_))
                    {
                        self.stderr = Some(stderr.clone().into());
                    }
                }
                _ => {}
            }
        }
        self.active_at = now();
        self.refresh_rows();
        let working = self.conversation.working();
        self.composer.update(cx, |c, cx| c.set_running(working, cx));
        if self.status != before {
            cx.emit(SessionEvent::Changed);
        }
        cx.notify();
    }

    /// Tells the list which rows to measure again.
    fn refresh_rows(&mut self) {
        let after: Vec<_> = self.conversation.items().iter().map(list_diff::fingerprint).collect();
        for (range, count) in list_diff::changes(&self.rows, &after) {
            self.list.splice(range, count);
        }
        self.rows = after;
    }

    fn command(&mut self, command: Command, cx: &mut Context<Self>) {
        let sent = match &self.session {
            Some(session) => session.send(command).map_err(|e| e.to_string()),
            None => Err("the session has ended".to_string()),
        };
        if let Err(why) = sent {
            self.problem = Some(format!("Not sent: {why}").into());
        }
        cx.notify();
    }

    /// Sends a message; the first one names the session.
    pub fn send(&mut self, text: String, cx: &mut Context<Self>) {
        if text.trim().is_empty() {
            return;
        }
        if self.conversation.items().is_empty() && self.id.is_none() {
            self.title = text.lines().next().unwrap_or("").chars().take(80).collect::<String>().into();
        }
        self.conversation.user_sent(text.clone());
        self.status = status::sent();
        self.problem = None;
        self.stderr = None;
        self.refresh_rows();
        self.list.scroll_to_end();
        match (&self.session, &self.id) {
            // The agent stopped (a crash, Stop): the session resumes, and the message goes then.
            (None, Some(id)) if !self.starting => {
                self.waiting_send = Some(text);
                self.starting = true;
                let id = id.clone();
                self.open(Some(id), false, cx);
            }
            (None, _) if self.starting => self.waiting_send = Some(text),
            // It never started (its program was missing, say): it tries again, and the message goes then.
            (None, None) => {
                self.waiting_send = Some(text);
                self.starting = true;
                self.open(None, false, cx);
            }
            _ => self.command(Command::send(text), cx),
        }
        cx.emit(SessionEvent::Changed);
    }

    pub fn interrupt(&mut self, cx: &mut Context<Self>) {
        if self.conversation.working() {
            self.command(Command::Interrupt, cx);
        }
    }

    /// Answers a waiting question with the choice of `kind` it offers.
    pub fn answer(&mut self, request: &lathe_agents::session::RequestId, kind: ChoiceKind, cx: &mut Context<Self>) {
        let choice = self.conversation.items().iter().find_map(|item| match item {
            Item::Permission { request: r, answer: Answer::Asking } if r.id == *request => {
                r.choices.iter().find(|c| c.kind == kind).map(|c| c.id.clone())
            }
            _ => None,
        });
        let Some(choice) = choice else { return };
        self.conversation.answered(request, kind);
        self.status = status::sent();
        self.refresh_rows();
        self.command(Command::Answer { request: request.clone(), choice }, cx);
        cx.emit(SessionEvent::Changed);
    }

    pub fn set_model(&mut self, model: String, cx: &mut Context<Self>) {
        self.model = Some(model.clone());
        if self.session.is_some() {
            self.command(Command::SetModel { model }, cx);
        }
    }

    pub fn set_mode(&mut self, mode: PermissionMode, cx: &mut Context<Self>) {
        self.mode = Some(mode);
        if self.session.is_some() {
            self.command(Command::SetPermissionMode { mode }, cx);
        }
    }

    /// Stops the agent: dropping its session ends it.
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.session = None;
        cx.notify();
    }

    pub fn running(&self) -> bool {
        self.session.is_some()
    }

    /// The reader looks at it now, or no longer.
    pub fn set_seen(&mut self, seen: bool, cx: &mut Context<Self>) {
        self.seen = seen;
        if seen {
            let now = status::opened(&self.status);
            if now != self.status {
                self.status = now;
                cx.emit(SessionEvent::Changed);
            }
        }
    }

    /// Starts renaming: an input with the name, which Enter keeps and Escape drops.
    pub fn start_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.shown_title();
        let input = cx.new(|cx| {
            let mut state = gpui_kit::component::input::InputState::new(window, cx);
            state.set_value(title, window, cx);
            state
        });
        input.update(cx, |i, cx| i.focus(window, cx));
        let sub = cx.subscribe(&input, |this, input, event: &gpui_kit::component::input::InputEvent, cx| match event {
            gpui_kit::component::input::InputEvent::PressEnter { .. } => {
                let name = input.read(cx).value().trim().to_string();
                this.renaming = None;
                this._renaming = None;
                if !name.is_empty() {
                    this.name = Some(name.into());
                    cx.emit(SessionEvent::Renamed);
                }
                cx.emit(SessionEvent::Changed);
                cx.notify();
            }
            gpui_kit::component::input::InputEvent::Blur => {
                this.renaming = None;
                this._renaming = None;
                cx.notify();
            }
            _ => {}
        });
        self.renaming = Some(input);
        self._renaming = Some(sub);
        cx.notify();
    }

    /// What the row and the tab say.
    pub fn shown_title(&self) -> SharedString {
        self.name.clone().unwrap_or_else(|| self.title.clone())
    }
}

/// The permission modes as the mode picker names them.
pub fn mode_word(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Ask => "Ask first",
        PermissionMode::AcceptEdits => "Accept edits",
        PermissionMode::Plan => "Plan",
        PermissionMode::Auto => "Auto",
        PermissionMode::Bypass => "Bypass permissions",
    }
}

fn mode_from(word: &str) -> Option<PermissionMode> {
    [PermissionMode::Ask, PermissionMode::AcceptEdits, PermissionMode::Plan, PermissionMode::Auto, PermissionMode::Bypass]
        .into_iter()
        .find(|m| mode_word(*m) == word)
}

/// A start that failed, in words that say what to do.
fn problem_words(error: &SessionError) -> String {
    match error {
        SessionError::Missing { program } => {
            format!("{program} is not installed on this host. Install it there, then start a new session.")
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests;
