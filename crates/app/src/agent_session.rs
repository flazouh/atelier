//! One agent session in a project: the agent's own session, the queue its events land in, the
//! `Conversation` they fold into, and the list that draws it.
//!
//! Events arrive on the agent's threads. The queue joins a block's deltas and wakes this entity once
//! when it goes from empty to not; the entity then drains everything waiting, folds it, and asks for
//! one repaint, so a fast stream costs a repaint a frame. Only the rows whose content changed are
//! measured again (`list_diff`), so the list keeps its scroll while text streams.
//!
//! Each turn is tracked for review (`lathe-review`): the turn's `begin` (a git snapshot) runs on a
//! background task before the message goes to the agent; every event then passes the tracker on the
//! agent's own thread, as it arrives, so a file is read before the tool that names it writes it; and the
//! turn's end `finish`es it there too. The panel shows the turn's changed files after its last row.

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
use lathe_review::{TurnReview, TurnTracker};
use std::sync::Mutex;

use crate::{
    list_diff,
    review_state::{Record, ReviewState, record_path},
    status,
};

/// How long after the review's last change it is written to the data folder.
const SAVE_AFTER: std::time::Duration = std::time::Duration::from_millis(500);

/// How far past the view the list lays out rows.
const OVERDRAW: f32 = 160.;

/// What the session tells the shell.
pub enum SessionEvent {
    /// Its title, status or id changed: the sidebar and the tabs draw it again.
    Changed,
    /// The reader asked to review a turn (`None` for the whole session) at a file.
    Review { turn: Option<usize>, path: Option<String> },
    /// The reader asked to open a file in the editor.
    OpenFile(String),
    /// The reader picked another agent for this session before its first message, by its backend's name.
    ChooseAgent(String),
    /// The reader pressed a pull request's chip in the agent's text.
    OpenPull(beui::PrChipData),
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
    waiting_send: Option<Command>,
    /// It resumes a past session, which keeps its agent.
    resumed: bool,
    /// Messages sent while the turn's tracker begins: `Some` from the begin until it lands, and they go
    /// after the first, in the same turn.
    beginning: Option<Vec<Command>>,
    pub list: ListState,
    /// What each row of the list draws, and its fingerprint.
    pub shown: Vec<list_diff::Row>,
    rows: Vec<(u8, usize, usize)>,
    /// The turn being recorded, shared with the sink on the agent's thread.
    tracker: Arc<Mutex<Option<TurnTracker>>>,
    /// Turns the sink finished, waiting for the next drain.
    finished: Arc<Mutex<Vec<TurnReview>>>,
    /// The pull requests a `#N` in the agent's text can name, from the project's list.
    pub pr_chips: std::rc::Rc<Vec<beui::PrChipData>>,
    /// The session's review: its turns, decisions, marks and comments, kept in the data folder.
    pub reviews: ReviewState,
    /// Writes the review to the data folder a moment after it last changed.
    _saving: Task<()>,
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
    /// The project's folder, as tool calls name paths under it.
    pub fn root(&self) -> String {
        self.project.root().display().to_string()
    }

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
        let tracker: Arc<Mutex<Option<TurnTracker>>> = Arc::default();
        let finished: Arc<Mutex<Vec<TurnReview>>> = Arc::default();
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
            beginning: None,
            resumed: resume.is_some(),
            // The list lays out this much past the view each frame: enough that a fast scroll never
            // shows an empty edge, little enough to stay inside a 120 Hz frame (docs/performance.md).
            list: ListState::new(0, ListAlignment::Bottom, px(OVERDRAW)),
            shown: Vec::new(),
            rows: Vec::new(),
            tracker,
            finished,
            pr_chips: std::rc::Rc::default(),
            reviews: ReviewState::default(),
            _saving: Task::ready(()),
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
        let (backend, project, sink) = (self.agent.backend.clone(), self.project.clone(), self.tracking_sink());
        let request = OpenRequest { resume: resume.clone(), model: self.model.clone(), mode: self.mode };
        let opening = cx.background_spawn(async move {
            let (history, record) = match &resume {
                Some(id) if read_history => {
                    // The review kept with the session; none, or one this build cannot read, is a fresh one.
                    let record = project.data_read(&record_path(&id.0)).ok().and_then(|bytes| serde_json::from_slice::<Record>(&bytes).ok());
                    (backend.history(project.as_ref(), id).unwrap_or_default(), record)
                }
                _ => (Vec::new(), None),
            };
            (history, record, backend.open(project, request, sink))
        });
        self._start = cx.spawn(async move |this, cx| {
            let (history, record, opened) = opening.await;
            _ = this.update(cx, |s, cx| {
                s.starting = false;
                for event in &history {
                    s.conversation.apply(event);
                }
                if let Some(record) = record {
                    s.reviews = ReviewState::from_record(record);
                }
                match opened {
                    Ok(session) => {
                        s.session = Some(session);
                        if let Some(command) = s.waiting_send.take() {
                            s.start_turn(command, cx);
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

    /// The sink the agent gets: each event passes the turn's tracker first, on the agent's thread, then
    /// goes to the queue. The turn's end finishes the tracker there, before its event reaches the UI.
    fn tracking_sink(&self) -> lathe_agents::session::EventSink {
        let (queue, tracker, finished, project) = (self.queue.sink(), self.tracker.clone(), self.finished.clone(), self.project.clone());
        Arc::new(move |event: Event| {
            {
                let mut tracker = tracker.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(t) = tracker.as_mut() {
                    t.observe(project.as_ref(), &event);
                }
                if matches!(event, Event::TurnEnded(_) | Event::Ended(_))
                    && let Some(t) = tracker.take()
                {
                    finished.lock().unwrap_or_else(|p| p.into_inner()).push(t.finish(project.as_ref()));
                }
            }
            queue(event);
        })
    }

    /// Folds every event waiting: one frame's worth.
    fn drain(&mut self, cx: &mut Context<Self>) {
        let events = self.queue.drain();
        if events.is_empty() {
            return;
        }
        let turns = std::mem::take(&mut *self.finished.lock().unwrap_or_else(|p| p.into_inner()));
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
        let ended_turns = !turns.is_empty();
        for turn in turns {
            self.reviews.finish_turn(turn, self.conversation.items().len());
        }
        if ended_turns {
            self.save_review(cx);
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
        let items = self.conversation.items();
        let shown = list_diff::rows(items.len(), &self.reviews.turn_marks);
        let after: Vec<_> = shown
            .iter()
            .map(|row| match *row {
                list_diff::Row::Item(ix) => list_diff::fingerprint(&items[ix]),
                list_diff::Row::Changes { turn } => list_diff::changes_fingerprint(turn),
            })
            .collect();
        for (range, count) in list_diff::changes(&self.rows, &after) {
            self.list.splice(range, count);
        }
        self.rows = after;
        self.shown = shown;
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
        // The first message of an empty conversation names it; a resumed one keeps its title.
        if self.conversation.items().is_empty() {
            self.title = text.lines().next().unwrap_or("").chars().take(80).collect::<String>().into();
        }
        self.conversation.user_sent(text.clone());
        self.status = status::sent();
        self.problem = None;
        self.stderr = None;
        self.refresh_rows();
        self.list.scroll_to_end();
        // The review comments go with the message, and show resolved once the agent's turn ends.
        let attachments = self.reviews.send_comments();
        if !attachments.is_empty() {
            self.save_review(cx);
        }
        let command = Command::Send { text, attachments };
        match (&self.session, &self.id) {
            // The agent stopped (a crash, Stop): the session resumes, and the message goes then.
            (None, Some(id)) if !self.starting => {
                self.waiting_send = Some(command);
                self.starting = true;
                let id = id.clone();
                self.open(Some(id), false, cx);
            }
            (None, _) if self.starting => self.waiting_send = Some(command),
            // It never started (its program was missing, say): it tries again, and the message goes then.
            (None, None) => {
                self.waiting_send = Some(command);
                self.starting = true;
                self.open(None, false, cx);
            }
            _ => self.start_turn(command, cx),
        }
        cx.emit(SessionEvent::Changed);
    }

    /// Sends a message that starts a turn: the turn's tracker begins first, off the UI thread, so it
    /// knows the files as they were before the agent reads the message. A message sent while a turn
    /// runs joins that turn.
    fn start_turn(&mut self, command: Command, cx: &mut Context<Self>) {
        if let Some(waiting) = &mut self.beginning {
            return waiting.push(command);
        }
        if self.tracker.lock().unwrap_or_else(|p| p.into_inner()).is_some() {
            return self.command(command, cx);
        }
        self.beginning = Some(Vec::new());
        let project = self.project.clone();
        let beginning = cx.background_spawn(async move { TurnTracker::begin(project.as_ref()) });
        cx.spawn(async move |this, cx| {
            let tracker = beginning.await;
            _ = this.update(cx, |s, cx| {
                *s.tracker.lock().unwrap_or_else(|p| p.into_inner()) = Some(tracker);
                s.command(command, cx);
                for joined in s.beginning.take().unwrap_or_default() {
                    s.command(joined, cx);
                }
            });
        })
        .detach();
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

    /// Writes the review to the project's data folder, a moment after its last change, off the UI thread.
    /// A session the agent has not named yet has nowhere to keep it.
    pub fn save_review(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.id.clone() else { return };
        let record = self.reviews.record();
        let project = self.project.clone();
        let timer = cx.background_executor().timer(SAVE_AFTER);
        let background = cx.background_executor().clone();
        self._saving = cx.spawn(async move |this, cx| {
            timer.await;
            let written = background
                .spawn(async move { serde_json::to_vec(&record).map_err(std::io::Error::other).and_then(|bytes| project.data_write(&record_path(&id.0), &bytes)) })
                .await;
            if let Err(error) = written {
                _ = this.update(cx, |s, cx| {
                    s.problem = Some(format!("The review was not kept: {error}").into());
                    cx.notify();
                });
            }
        });
    }

    /// A new session takes another agent until its first message: then its conversation belongs to one.
    pub fn can_choose_agent(&self) -> bool {
        !self.resumed && self.conversation.items().is_empty() && self.waiting_send.is_none()
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
