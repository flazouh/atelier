use std::sync::Arc;
use std::{collections::HashMap, time::Instant};
use std::sync::Mutex;

use atelier_ui::session_status::SessionStatus;
use futures_channel::mpsc;
use futures_util::StreamExt;
use atelier_ui::{PromptInput, PromptInputEvent, PromptModel};
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, ListAlignment, ListState, SharedString,
    Subscription, Task, Window,
};
use atelier_ui::scale::px;
use atelier_agents::{
    registry::Agent,
    session::{Answer, ChoiceKind, Command, Conversation, Event, EventQueue, Item, OpenRequest, PermissionMode, Session, SessionId},
};
use atelier_project::Project;
use atelier_review::{TurnReview, TurnTracker};

use crate::{
    list_diff,
    review_state::{Record, ReviewState, record_path},
    status,
};
use super::types::{OVERDRAW, SAVE_AFTER, SessionEvent};
use super::helpers::{is_activity, mode_from, mode_word, now, problem_words};

impl EventEmitter<SessionEvent> for AgentSession {}

pub struct AgentSession {
    /// The panel's id: stable from the start, before the agent names the session.
    pub key: SharedString,
    pub agent: Agent,
    pub(super) project: Arc<dyn Project>,
    pub(super) session: Option<Box<dyn Session>>,
    pub(super) queue: EventQueue,
    pub conversation: Conversation,
    /// The agent's id for the session, once it says it.
    pub id: Option<SessionId>,
    pub title: SharedString,
    /// A name the reader gave it, over the first message.
    pub name: Option<SharedString>,
    pub status: SessionStatus,
    pub active_at: u64,
    /// Whether `active_at` is the agent's own time, from its work or the record: the agent's list then
    /// has no say.
    pub activity_known: bool,
    /// Asked the agent for a short title already, so it is asked once.
    pub(super) titled: bool,
    pub(super) _titling: Task<()>,
    /// The reader is looking at it: a turn that ends is seen.
    pub seen: bool,
    pub model: Option<String>,
    pub mode: Option<PermissionMode>,
    /// The task the session began from, for the header's chip and the first signal.
    pub task: Option<crate::tasks::TaskRef>,
    /// The task heard that the session started.
    pub(crate) task_told: bool,
    /// Why it could not start, or a message that did not go.
    pub problem: Option<SharedString>,
    /// What the agent wrote to stderr as it failed: the tail behind "Show details".
    pub stderr: Option<SharedString>,
    /// Starting, or reading its history: the panel shows it.
    pub starting: bool,
    /// A message written while the agent was not running: it goes once the session resumes.
    pub(super) waiting_send: Option<Command>,
    /// It resumes a past session, which keeps its agent.
    pub(super) resumed: bool,
    /// Messages sent while the turn's tracker begins: `Some` from the begin until it lands, and they go
    /// after the first, in the same turn.
    pub(super) beginning: Option<Vec<Command>>,
    pub list: ListState,
    /// What each row of the list draws, and its fingerprint.
    pub shown: Vec<list_diff::Row>,
    pub(super) rows: Vec<(u8, usize, usize)>,
    /// The turn being recorded, shared with the sink on the agent's thread.
    pub(super) tracker: Arc<Mutex<Option<TurnTracker>>>,
    /// Turns the sink finished, waiting for the next drain.
    pub(super) finished: Arc<Mutex<Vec<TurnReview>>>,
    /// The pull requests a `#N` in the agent's text can name, from the project's list.
    pub pr_chips: std::rc::Rc<Vec<atelier_ui::PrChipData>>,
    /// The session's review: its turns, decisions, marks and comments, kept in the data folder.
    pub reviews: ReviewState,
    /// The card of the pull request the session opened, above the composer.
    pub pull_card: Option<Entity<crate::pull_card::PullCard>>,
    pub(super) _pull_card: Option<Subscription>,
    /// Writes the review to the data folder a moment after it last changed.
    pub(super) _saving: Task<()>,
    pub composer: Entity<PromptInput>,
    /// The project's badge, as the sidebar draws it.
    pub badge: Option<atelier_ui::sidebar_model::Badge>,
    /// The activity groups the reader opened, by the index of their first item.
    pub opened_groups: std::collections::HashSet<usize>,
    /// What the project adds to the `/` list, and the agent's own commands once it has said them.
    pub(super) project_commands: Vec<atelier_agents::commands::CommandInfo>,
    pub(super) agent_commands: Vec<String>,
    pub(super) _lists: Task<()>,
    /// The name being typed, while the reader renames the session.
    pub renaming: Option<Entity<gpui_kit::component::input::InputState>>,
    pub(super) _renaming: Option<Subscription>,
    /// When each thinking block began, for its live "Thinking for 12s".
    pub thinking_since: HashMap<atelier_agents::session::BlockId, Instant>,
    pub(super) _composer: Subscription,
    pub(super) _pump: Task<()>,
    pub(super) _start: Task<()>,
}

impl AgentSession {
    /// Gives the session its project's badge, which its panel's head shows. The shell sets it whenever it syncs.
    pub fn set_badge(&mut self, badge: atelier_ui::sidebar_model::Badge, cx: &mut Context<Self>) {
        if self.badge.as_ref() != Some(&badge) {
            self.badge = Some(badge);
            cx.notify();
        }
    }

    /// The name of the project's folder: the panel names the project it works in.
    pub fn project_name(&self) -> SharedString {
        self.project.root().file_name().map_or_else(|| self.project.root().display().to_string(), |n| n.to_string_lossy().into_owned()).into()
    }

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
                match atelier_agents::registry::model_mark(&m.id) {
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
            PromptInputEvent::Command { name, args } => this.run_command(name, args, cx),
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
            // A resumed session's last activity comes from the agent's list; a new one starts now.
            active_at: if resume.is_some() { 0 } else { now() },
            activity_known: resume.is_none(),
            titled: false,
            _titling: Task::ready(()),
            seen: false,
            model: None,
            mode: None,
            task: None,
            task_told: false,
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
            pull_card: None,
            _pull_card: None,
            _saving: Task::ready(()),
            composer,
            badge: None,
            opened_groups: std::collections::HashSet::new(),
            project_commands: Vec::new(),
            agent_commands: Vec::new(),
            _lists: Task::ready(()),
            renaming: None,
            _renaming: None,
            thinking_since: HashMap::new(),
            _composer,
            _pump,
            _start: Task::ready(()),
        };
        // The conversation follows the agent's output while the reader is at its end, lets go when they scroll up, and takes
        // hold again when they come back (beui's message-scroller `followOutput`). The panel is told of each scroll, so its
        // "Latest" button and its rail keep up.
        this.list.set_follow_mode(gpui_kit::FollowMode::Tail);
        let scrolled = cx.entity().downgrade();
        this.list.set_scroll_handler(move |_, _, cx| {
            scrolled.update(cx, |_, cx| cx.notify()).ok();
        });
        this.open(resume.map(|(id, _)| id), true, cx);
        this.read_lists(cx);
        this
    }

    /// Reads the project's skills, command files and files off the UI thread, for the composer's lists.
    pub(super) fn read_lists(&mut self, cx: &mut Context<Self>) {
        let project = self.project.clone();
        let reading = cx.background_spawn(async move { super::composer_lists::read(project.as_ref()) });
        self._lists = cx.spawn(async move |this, cx| {
            let lists = reading.await;
            _ = this.update(cx, |s, cx| {
                s.project_commands = lists.project_commands;
                s.composer.update(cx, |c, cx| c.set_files(lists.files.into_iter().map(Into::into).collect(), cx));
                s.offer_commands(cx);
            });
        });
    }

    /// Gives the composer the `/` list: atelier's, the project's and the agent's own.
    pub(super) fn offer_commands(&mut self, cx: &mut Context<Self>) {
        let items = super::composer_lists::commands(self.project_commands.clone(), &self.agent_commands)
            .into_iter()
            .map(|c| atelier_ui::command_item::CommandItem {
                name: c.name.into(),
                source: match c.source {
                    atelier_agents::commands::CommandSource::Agent => atelier_ui::command_item::CommandSource::Agent,
                    atelier_agents::commands::CommandSource::Atelier => atelier_ui::command_item::CommandSource::Atelier,
                    atelier_agents::commands::CommandSource::Skill => atelier_ui::command_item::CommandSource::Skill,
                },
                summary: c.summary.into(),
                args_hint: c.args_hint.map(Into::into),
            })
            .collect();
        self.composer.update(cx, |c, cx| c.set_commands(items, cx));
    }

    /// A `/` command the reader chose: atelier's own runs here, any other goes to the agent as its text.
    pub(super) fn run_command(&mut self, name: &str, args: &str, cx: &mut Context<Self>) {
        match name {
            "files" if super::composer_lists::atelier_runs(name) => cx.emit(SessionEvent::ShowFiles),
            "tasks" if super::composer_lists::atelier_runs(name) => cx.emit(SessionEvent::ShowTasks),
            "review" if super::composer_lists::atelier_runs(name) => cx.emit(SessionEvent::Review { turn: None, path: None }),
            _ => self.send(super::composer_lists::agent_text(name, args), cx),
        }
    }

    /// Opens the agent's session off the UI thread: its history first when it resumes and the panel
    /// has not got it (`read_history`), then the agent.
    pub(super) fn open(&mut self, resume: Option<SessionId>, read_history: bool, cx: &mut Context<Self>) {
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
                if !history.is_empty() {
                    cx.emit(SessionEvent::TextSettled);
                }
                if let Some(record) = record {
                    s.reviews = ReviewState::from_record(record);
                    // The record knows when the agent last worked; the agent's list, which a resume
                    // touches, does not.
                    if let (Some(at), false) = (s.reviews.last_activity, s.activity_known) {
                        s.active_at = at;
                        s.activity_known = true;
                        cx.emit(SessionEvent::Changed);
                    }
                    if let Some(reference) = s.reviews.pull.clone() {
                        s.show_card(reference, cx);
                    }
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
                        s.status = SessionStatus::Failed(atelier_ui::session_status::short_reason(&problem_words(&error)));
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
    pub(super) fn tracking_sink(&self) -> atelier_agents::session::EventSink {
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
    pub(super) fn drain(&mut self, cx: &mut Context<Self>) {
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
                    if self.agent_commands != started.commands {
                        self.agent_commands = started.commands.clone();
                        self.offer_commands(cx);
                    }
                    if self.task.is_some() && !self.task_told {
                        self.task_told = true;
                        cx.emit(SessionEvent::Task(crate::tasks::signal::TaskEvent::Started));
                    } else if self.task.is_none() {
                        cx.emit(SessionEvent::Task(crate::tasks::signal::TaskEvent::Adopt));
                    }
                }
                Event::TurnEnded(end) => {
                    let ok = matches!(end.outcome, atelier_agents::session::TurnOutcome::Completed);
                    cx.emit(SessionEvent::Task(crate::tasks::signal::TaskEvent::TurnEnded { ok }));
                }
                Event::Ended(end) => {
                    self.session = None;
                    if let atelier_agents::session::EndReason::Exited { stderr, .. } = end
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
            cx.emit(SessionEvent::TextSettled);
            self.draft_title(cx);
        }
        if events.iter().any(is_activity) {
            self.active_at = now();
            self.reviews.last_activity = Some(self.active_at);
            self.activity_known = true;
            // Kept now, not only at a turn's end: a turn cut off (the app closed during a question)
            // must still say when the agent last worked, or a resume takes the agent's list, which
            // the resume touches, and the row says "now". The save waits for a pause.
            if !ended_turns {
                self.save_review(cx);
            }
        }
        self.refresh_rows();
        let working = self.conversation.working();
        self.composer.update(cx, |c, cx| c.set_running(working, cx));
        if self.status != before {
            cx.emit(SessionEvent::Changed);
        }
        cx.notify();
    }

    /// Whether the group of items `from..to` is the live one: the agent works and nothing comes after it.
    pub fn group_is_live(&self, to: usize) -> bool {
        self.conversation.working() && to == self.conversation.items().len()
    }

    /// Whether group `from` shows its items: the live one always does, a finished one when the reader opened it.
    pub fn group_is_open(&self, from: usize, to: usize) -> bool {
        self.group_is_live(to) || self.opened_groups.contains(&from)
    }

    /// Opens or folds the finished group that starts at item `from`.
    pub fn toggle_group(&mut self, from: usize, cx: &mut Context<Self>) {
        if !self.opened_groups.remove(&from) {
            self.opened_groups.insert(from);
        }
        self.refresh_rows();
        cx.notify();
    }

    /// Tells the list which rows to measure again.
    pub(crate) fn refresh_rows(&mut self) {
        let items = self.conversation.items();
        let shown = list_diff::grouped(items, &|ix| crate::session_view::calls::shows(items, ix), &self.reviews.turn_marks);
        let after: Vec<_> = shown
            .iter()
            .map(|row| match *row {
                list_diff::Row::Item(ix) => list_diff::fingerprint(&items[ix]),
                list_diff::Row::Changes { turn } => list_diff::changes_fingerprint(turn),
                list_diff::Row::Activity { from, to } => list_diff::activity_fingerprint(items, from, to, self.group_is_live(to), self.group_is_open(from, to)),
            })
            .collect();
        for (range, count) in list_diff::changes(&self.rows, &after) {
            self.list.splice(range, count);
        }
        self.rows = after;
        self.shown = shown;
    }

    pub(super) fn command(&mut self, command: Command, cx: &mut Context<Self>) {
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
    /// Tells the tasks linked to this session that something happened in it.
    pub fn tell_task(&mut self, event: crate::tasks::signal::TaskEvent, cx: &mut Context<Self>) {
        cx.emit(SessionEvent::Task(event));
    }

    pub fn send(&mut self, text: String, cx: &mut Context<Self>) {
        if text.trim().is_empty() {
            return;
        }
        if !self.conversation.items().is_empty() && self.id.is_some() {
            cx.emit(SessionEvent::Task(crate::tasks::signal::TaskEvent::Replied));
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
        // A message sent goes to the end and the follow takes hold again.
        self.list.set_follow_mode(gpui_kit::FollowMode::Tail);
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
    pub(super) fn start_turn(&mut self, command: Command, cx: &mut Context<Self>) {
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
    pub fn answer(&mut self, request: &atelier_agents::session::RequestId, kind: ChoiceKind, cx: &mut Context<Self>) {
        let choice = self.conversation.items().iter().find_map(|item| match item {
            Item::Permission { request: r, answer: Answer::Asking } if r.id == *request => {
                r.choices.iter().find(|c| c.kind == kind).map(|c| c.id.clone())
            }
            _ => None,
        });
        let Some(choice) = choice else { return };
        // Kept with the review, so the call's row keeps its mark after a resume.
        let call = self.conversation.items().iter().find_map(|item| match item {
            Item::Permission { request: r, .. } if r.id == *request => Some(r.call.id.as_str().to_string()),
            _ => None,
        });
        if let Some(call) = call {
            self.reviews.approvals.insert(call, crate::review_state::Approval::of(kind));
            self.save_review(cx);
        }
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

    /// The session opened `reference`, or its branch had it: it is kept, and its card shows.
    pub fn set_pull(&mut self, reference: atelier_forge::PullRef, cx: &mut Context<Self>) {
        self.reviews.pull = Some(reference.clone());
        self.save_review(cx);
        self.show_card(reference, cx);
    }

    pub(super) fn show_card(&mut self, reference: atelier_forge::PullRef, cx: &mut Context<Self>) {
        if self.pull_card.as_ref().is_some_and(|c| *c.read(cx).reference() == reference) {
            return;
        }
        // Tests never reach a forge: their cards read an empty one.
        #[cfg(test)]
        let forge: Arc<dyn atelier_forge::Forge> = Arc::new(atelier_pr_view::fixture::FixtureForge::new());
        #[cfg(not(test))]
        let forge: Arc<dyn atelier_forge::Forge> = Arc::new(atelier_forge::github::GitHub::new(self.project.clone()));
        let card = cx.new(|cx| crate::pull_card::PullCard::new(reference, forge, cx));
        self._pull_card = Some(cx.subscribe(&card, |_, _, event: &crate::pull_card::CardEvent, cx| match event {
            crate::pull_card::CardEvent::Show(reference) => cx.emit(SessionEvent::ShowPull(reference.clone())),
        }));
        self.pull_card = Some(card);
        cx.notify();
    }

    /// Asks the agent, once, for a short title from the first exchange, off the UI thread. A name the
    /// reader gave, before or while it drafts, stays.
    pub(super) fn draft_title(&mut self, cx: &mut Context<Self>) {
        if self.name.is_some() || self.titled {
            return;
        }
        let items = self.conversation.items();
        let asked = items.iter().find_map(|i| match i {
            Item::User { text } => Some(text.clone()),
            _ => None,
        });
        let answered = items.iter().find_map(|i| match i {
            Item::Text { text, .. } => Some(text.clone()),
            _ => None,
        });
        let Some(asked) = asked else { return };
        self.titled = true;
        let prompt = crate::session_title::prompt(&asked, answered.as_deref().unwrap_or(""));
        let (backend, project, model) = (self.agent.backend.clone(), self.project.clone(), self.model.clone());
        let drafting = cx.background_spawn(async move { backend.draft(project.as_ref(), &prompt, model.as_deref()) });
        self._titling = cx.spawn(async move |this, cx| {
            let Some(title) = drafting.await.ok().and_then(|d| crate::session_title::clean(&d)) else { return };
            _ = this.update(cx, |s, cx| {
                if s.name.is_none() {
                    s.name = Some(title.into());
                    cx.emit(SessionEvent::Renamed);
                    cx.emit(SessionEvent::Changed);
                    cx.notify();
                }
            });
        });
    }

    /// The agent's text, message by message.
    pub fn agent_texts(&self) -> Vec<String> {
        self.conversation.items().iter().filter_map(|item| match item {
            Item::Text { text, .. } => Some(text.clone()),
            _ => None,
        }).collect()
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
