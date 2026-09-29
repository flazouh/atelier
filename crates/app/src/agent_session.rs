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
    pub list: ListState,
    rows: Vec<(u8, usize, usize)>,
    pub composer: Entity<PromptInput>,
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
        let composer = cx.new(|cx| PromptInput::new(format!("Ask {}", agent.name), "", window, cx).models(models));
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
            list: ListState::new(0, ListAlignment::Bottom, px(600.)),
            rows: Vec::new(),
            composer,
            thinking_since: HashMap::new(),
            _composer,
            _pump,
            _start: Task::ready(()),
        };
        this.open(resume.map(|(id, _)| id), cx);
        this
    }

    /// Opens the agent's session off the UI thread: its history first when it resumes, then the agent.
    fn open(&mut self, resume: Option<SessionId>, cx: &mut Context<Self>) {
        let (backend, project, sink) = (self.agent.backend.clone(), self.project.clone(), self.queue.sink());
        let request = OpenRequest { resume: resume.clone(), model: self.model.clone(), mode: self.mode };
        let opening = cx.background_spawn(async move {
            let history = match &resume {
                Some(id) => backend.history(project.as_ref(), id).unwrap_or_default(),
                None => Vec::new(),
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
                    Ok(session) => s.session = Some(session),
                    Err(error) => s.problem = Some(problem_words(&error).into()),
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
        self.refresh_rows();
        self.list.scroll_to_end();
        self.command(Command::send(text), cx);
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
