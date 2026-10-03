use std::sync::Arc;
use std::{collections::HashMap, time::Instant};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

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
    session::{Answer, ChoiceKind, Command, ContextFill, Conversation, Event, EventQueue, Item, OpenRequest, PermissionMode, Session, SessionId},
};
use atelier_project::Project;
use atelier_review::{TurnReview, TurnTracker};

use crate::{
    list_diff,
    review_state::{Record, ReviewState, record_path},
    status,
    tool_density::{ToolDensity, tool_density},
};
use super::types::{ARRIVAL_KEPT, OVERDRAW, SAVE_AFTER, SessionEvent};
use super::helpers::{is_activity, mode_from, mode_word, now, problem_words};

impl EventEmitter<SessionEvent> for AgentSession {}

/// Whether a skill picked from a composer's `/` list runs at once, as the settings say.
pub(crate) struct RunPickedSkills(pub bool);

impl gpui_kit::Global for RunPickedSkills {}

pub(crate) fn runs_picked_skills(cx: &gpui_kit::App) -> bool {
    cx.try_global::<RunPickedSkills>().is_some_and(|r| r.0)
}

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
    titled: bool,
    _titling: Task<()>,
    pub(super) limit_clock: Task<()>,
    /// The sign-in of its agent, when it has none.
    pub(super) signer: super::sign_in::Signer,
    /// The reader is looking at it: a turn that ends is seen.
    pub seen: bool,
    pub model: Option<String>,
    pub mode: Option<PermissionMode>,
    /// What its agent runs on, for an agent with a choice; `None` leaves the agent as the host has it, and a resume
    /// finds the account that holds the session.
    pub provider: Option<crate::providers::Choice>,
    /// The session this one continues, until its first message goes.
    pub(super) handoff: Option<super::handoff::Handoff>,
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
    waiting_send: Option<Command>,
    /// It resumes a past session, which keeps its agent.
    pub(super) resumed: bool,
    /// Messages sent while the turn's tracker begins: `Some` from the begin until it lands, and they go
    /// after the first, in the same turn.
    beginning: Option<Vec<Command>>,
    /// Messages held for after the running turn, oldest first: the next goes when a turn completes.
    pub(super) queued: Vec<String>,
    /// Whether the running turn is one the reader's message started.
    pub(super) asked_turn: bool,
    pub list: ListState,
    /// The list's follow of the output and its glides.
    pub glide: crate::glide::Glide,
    /// What each row of the list draws, and its fingerprint.
    pub shown: Vec<list_diff::Row>,
    /// When each row that came in live arrived, for its entrance; a loaded conversation has none.
    pub arrived: HashMap<list_diff::Arrival, std::time::Instant>,
    pub(super) rows: Vec<(u8, usize, usize)>,
    /// The number of the latest open. An agent a later open replaced still runs a moment, and its events, its end
    /// above all, are not this session's.
    live_open: Arc<AtomicU64>,
    /// The turn being recorded, shared with the sink on the agent's thread.
    pub(super) tracker: Arc<Mutex<Option<TurnTracker>>>,
    /// Turns the sink finished, waiting for the next drain.
    pub(super) finished: Arc<Mutex<Vec<TurnReview>>>,
    /// The pull requests a `#N` in the agent's text can name, from the project's list.
    pub pr_chips: std::rc::Rc<Vec<atelier_ui::PrChipData>>,
    /// The session's review: its turns, decisions, marks and comments, kept in the data folder.
    pub reviews: ReviewState,
    /// The files the whole session changed, each against its text before the session, above the composer.
    changed: std::rc::Rc<Vec<atelier_ui::ChangedFile>>,
    _changed: Task<()>,
    /// The card of the pull request the session opened, above the composer.
    pub pull_card: Option<Entity<crate::pull_card::PullCard>>,
    _pull_card: Option<Subscription>,
    /// Writes the review to the data folder a moment after it last changed.
    _saving: Task<()>,
    pub composer: Entity<PromptInput>,
    /// Dictation: the cues, and the clearing of a failed press's words.
    pub(super) dictation: super::dictation::Dictation,
    /// The project's badge, as the sidebar draws it.
    pub badge: Option<atelier_ui::sidebar_model::Badge>,
    /// The activity groups the reader opened, by the index of their first item.
    pub opened_groups: std::collections::HashSet<usize>,
    /// What the project adds to the `/` list, and the agent's own commands once it has said them.
    pub(super) project_commands: Vec<atelier_agents::commands::CommandInfo>,
    agent_commands: Vec<String>,
    _lists: Task<()>,
    /// The agent's accounts on the project's host, for the provider choice.
    pub provider_accounts: Vec<atelier_agents::session::Account>,
    /// An OpenRouter key is kept, so OpenRouter is offered.
    pub key_kept: bool,
    _providers: Task<()>,
    /// Where the project says this session can be handed off to, for the limit notice's menu.
    pub handoff_branches: Vec<atelier_ui::menu::Branch>,
    /// The name being typed, while the reader renames the session.
    pub renaming: Option<Entity<gpui_kit::component::input::InputState>>,
    _renaming: Option<Subscription>,
    /// When each thinking block began, for its live "Thinking for 12s".
    pub thinking_since: HashMap<atelier_agents::session::BlockId, Instant>,
    _composer: Subscription,
    _skills: Subscription,
    _key: Subscription,
    _away: Subscription,
    /// How the list shows tool calls, as the Settings page set it: [`ToolDensity::Grouped`] folds a run of work into one row.
    density: ToolDensity,
    _density: Subscription,
    _pump: Task<()>,
    _start: Task<()>,
}

impl AgentSession {
    /// The files the whole session changed, as the list above the composer shows them.
    pub fn changed_files(&self) -> std::rc::Rc<Vec<atelier_ui::ChangedFile>> {
        self.changed.clone()
    }

    /// Diffs the whole session off the UI thread, for [`Self::changed_files`].
    fn diff_session(&mut self, cx: &mut Context<Self>) {
        let turns = self.reviews.turns.clone();
        let whole = cx.background_spawn(async move { atelier_review::present::changed_files(&turns.whole()) });
        self._changed = cx.spawn(async move |this, cx| {
            let files = whole.await;
            _ = this.update(cx, |s, cx| {
                s.changed = std::rc::Rc::new(files);
                cx.notify();
            });
        });
    }

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
        let composer = cx.new(|cx| {
            let mut input = PromptInput::new(format!("Ask {}", agent.name), "", window, cx).models(models).modes(modes);
            input.set_dictation(true, cx);
            super::dictation::start_up(&mut input, cx);
            input.set_run_picked_skills(runs_picked_skills(cx));
            input
        });
        let _skills = cx.observe_global::<RunPickedSkills>(|this: &mut Self, cx| {
            let run = runs_picked_skills(cx);
            this.composer.update(cx, |c, _| c.set_run_picked_skills(run));
        });
        let _density = cx.observe_global::<ToolDensity>(|this: &mut Self, cx| {
            let density = tool_density(cx);
            if density == this.density {
                return;
            }
            this.density = density;
            // Every row is drawn another way: the list measures them all again, and none of them arrives anew.
            this.list.splice(0..this.rows.len(), 0);
            this.rows.clear();
            this.refresh_rows();
            this.arrived.clear();
            cx.notify();
        });
        let _composer = cx.subscribe_in(&composer, window, |this, _, event: &PromptInputEvent, window, cx| match event {
            PromptInputEvent::Submit(text) => this.send(text.to_string(), cx),
            PromptInputEvent::Stop => this.interrupt(cx),
            PromptInputEvent::Queue(text) => this.queue(text.to_string(), cx),
            PromptInputEvent::Unqueue(place) => this.unqueue(*place, cx),
            PromptInputEvent::SendQueued(place) => this.send_queued(*place, cx),
            PromptInputEvent::ModelChanged(model) => this.set_model(model.to_string(), cx),
            PromptInputEvent::ModeChanged(mode) => {
                if let Some(mode) = mode_from(mode) {
                    this.set_mode(mode, cx)
                }
            }
            PromptInputEvent::DictationStart => this.dictation_start(window, cx),
            PromptInputEvent::DictationStop => this.dictation_stop(cx),
            PromptInputEvent::DictationCancel => this.dictation_cancel(cx),
            PromptInputEvent::DictationDiscard => this.dictation_discard(cx),
            PromptInputEvent::DictationDevices => this.dictation_devices(cx),
            PromptInputEvent::DictationDevice(id) => this.dictation_device(id.clone(), cx),
            PromptInputEvent::DictationHold(hold) => this.dictation_hold(*hold, cx),
            PromptInputEvent::Action(_) => {}
            PromptInputEvent::Command { name, args } => this.run_command(name, args, cx),
        });
        super::dictation::warm(cx);
        let _key = super::dictation::hear_key(&composer, window, cx);
        super::dictation::take_recovered(key.clone(), cx.weak_entity(), window.window_handle(), cx);
        let _away = cx.observe_window_activation(window, |_, window, cx| {
            if !window.is_window_active() {
                super::dictation::key_away(cx);
            }
        });
        let agent_has_providers = agent.backend.capabilities().providers;
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
            limit_clock: Task::ready(()),
            signer: Default::default(),
            seen: false,
            model: None,
            mode: None,
            provider: (resume.is_none() && agent_has_providers).then(|| crate::providers::default_choice(cx)),
            handoff: None,
            task: None,
            task_told: false,
            problem: None,
            stderr: None,
            starting: true,
            waiting_send: None,
            beginning: None,
            queued: Vec::new(),
            asked_turn: false,
            resumed: resume.is_some(),
            // The list lays out this much past the view each frame: enough that a fast scroll never
            // shows an empty edge, little enough to stay inside a 120 Hz frame (docs/performance.md).
            list: ListState::new(0, ListAlignment::Bottom, px(OVERDRAW)),
            glide: crate::glide::Glide::default(),
            shown: Vec::new(),
            arrived: HashMap::new(),
            rows: Vec::new(),
            live_open: Arc::default(),
            tracker,
            finished,
            pr_chips: std::rc::Rc::default(),
            reviews: ReviewState::default(),
            pull_card: None,
            _pull_card: None,
            _saving: Task::ready(()),
            composer,
            dictation: super::dictation::Dictation::new(),
            badge: None,
            opened_groups: std::collections::HashSet::new(),
            project_commands: Vec::new(),
            agent_commands: Vec::new(),
            _lists: Task::ready(()),
            provider_accounts: Vec::new(),
            key_kept: false,
            _providers: Task::ready(()),
            handoff_branches: Vec::new(),
            renaming: None,
            _renaming: None,
            thinking_since: HashMap::new(),
            _composer,
            _skills,
            _key,
            _away,
            density: tool_density(cx),
            _density,
            changed: std::rc::Rc::default(),
            _changed: Task::ready(()),
            _pump,
            _start: Task::ready(()),
        };
        // The conversation follows the agent's output while the reader is at its end, lets go when they scroll up, and takes
        // hold again when they come back (beui's message-scroller `followOutput`). The panel is told of each scroll, so its
        // "Latest" button and its rail keep up.
        let (scrolled, glide) = (cx.entity().downgrade(), this.glide.clone());
        this.list.set_scroll_handler(move |_, _, cx| {
            glide.scrolled();
            scrolled.update(cx, |_, cx| cx.notify()).ok();
        });
        this.open(resume.map(|(id, _)| id), true, cx);
        this.read_lists(cx);
        if this.provider.is_some() {
            this.read_provider_choices(cx);
        }
        this
    }

    /// Reads the project's skills, command files and files off the UI thread, for the composer's lists.
    fn read_lists(&mut self, cx: &mut Context<Self>) {
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

    /// Reads the host's accounts and whether an OpenRouter key is kept, off the UI thread, for the provider choice.
    fn read_provider_choices(&mut self, cx: &mut Context<Self>) {
        let (backend, project, secrets) = (self.agent.backend.clone(), self.project.clone(), crate::providers::secrets(cx));
        let reading = cx.background_spawn(async move {
            let accounts = backend.accounts(project.as_ref()).unwrap_or_default();
            let key_kept = matches!(secrets.read(atelier_settings::secrets::OPENROUTER_KEY), Ok(Some(_)));
            (accounts, key_kept)
        });
        self._providers = cx.spawn(async move |this, cx| {
            let (accounts, key_kept) = reading.await;
            _ = this.update(cx, |s, cx| {
                s.provider_accounts = accounts;
                s.key_kept = key_kept;
                cx.notify();
            });
        });
    }

    /// The providers the session can switch to: the signed-in accounts, OpenRouter when a key is kept, and the one
    /// it is on.
    pub fn provider_choices(&self) -> Vec<crate::providers::Choice> {
        use crate::providers::Choice;
        let mut choices: Vec<Choice> = self.provider_accounts.iter().filter(|a| a.signed_in).map(|a| Choice::Account(a.name.clone())).collect();
        if self.key_kept {
            choices.push(Choice::OpenRouter);
        }
        if let Some(current) = &self.provider
            && !choices.contains(current)
        {
            choices.insert(0, current.clone());
        }
        choices
    }

    /// Gives the composer the `/` list: atelier's, the project's and the agent's own.
    fn offer_commands(&mut self, cx: &mut Context<Self>) {
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
            "login" if super::composer_lists::atelier_runs(name) => self.sign_in(cx),
            "review" if super::composer_lists::atelier_runs(name) => cx.emit(SessionEvent::Review { turn: None, path: None }),
            _ => self.send(super::composer_lists::agent_text(name, args), cx),
        }
    }

    /// Opens the agent's session off the UI thread: its history first when it resumes and the panel
    /// has not got it (`read_history`), then the agent.
    pub(super) fn open(&mut self, resume: Option<SessionId>, read_history: bool, cx: &mut Context<Self>) {
        let this_open = self.live_open.fetch_add(1, Ordering::SeqCst) + 1;
        let (backend, project, sink) = (self.agent.backend.clone(), self.project.clone(), self.tracking_sink(this_open));
        let fork = self.native_fork().filter(|_| resume.is_none());
        let mut request = OpenRequest { resume: resume.clone().or(fork.clone()), model: self.model.clone(), mode: self.mode, provider: None, fork: fork.is_some() };
        let (choice, secrets) = (self.provider.clone(), crate::providers::secrets(cx));
        let opening = cx.background_spawn(async move {
            let provider = choice.map(|choice| crate::providers::provider(&choice, secrets.as_ref())).transpose();
            let (history, record) = match &resume {
                Some(id) if read_history => {
                    // The review kept with the session; none, or one this build cannot read, is a fresh one.
                    let record = project.data_read(&record_path(&id.0)).ok().and_then(|bytes| serde_json::from_slice::<Record>(&bytes).ok());
                    (backend.history(project.as_ref(), id).unwrap_or_default(), record)
                }
                _ => (Vec::new(), None),
            };
            let opened = match provider {
                Ok(provider) => {
                    request.provider = provider;
                    backend.open(project, request, sink)
                }
                Err(why) => Err(atelier_agents::session::SessionError::Start(why)),
            };
            (history, record, opened)
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
                    s.diff_session(cx);
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
                if !history.is_empty() {
                    // A loaded conversation is there at once; only what comes in live enters.
                    s.arrived.clear();
                }
                cx.emit(SessionEvent::Changed);
                cx.notify();
            });
        });
    }

    /// The sink the agent gets: each event passes the turn's tracker first, on the agent's thread, then
    /// goes to the queue. The turn's end finishes the tracker there, before its event reaches the UI.
    fn tracking_sink(&self, this_open: u64) -> atelier_agents::session::EventSink {
        let (queue, tracker, finished, project) = (self.queue.sink(), self.tracker.clone(), self.finished.clone(), self.project.clone());
        let live_open = self.live_open.clone();
        Arc::new(move |event: Event| {
            if live_open.load(Ordering::SeqCst) != this_open {
                return;
            }
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
                Event::Limit(_) => self.watch_limit(cx),
                Event::TurnEnded(end) => {
                    let ok = matches!(end.outcome, atelier_agents::session::TurnOutcome::Completed);
                    self.turn_ran(ok, cx);
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
            self.diff_session(cx);
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
        self.after_turn(&events, cx);
        self.refresh_rows();
        let working = self.conversation.working();
        self.composer.update(cx, |c, cx| c.set_running(working, cx));
        self.show_context(cx);
        if self.status != before {
            cx.emit(SessionEvent::Changed);
        }
        cx.notify();
    }

    /// Tells the composer how full the agent's context is, once the agent has told its window too.
    fn show_context(&self, cx: &mut Context<Self>) {
        let ContextFill { used, window: Some(window) } = self.conversation.context() else { return };
        self.composer.update(cx, |c, cx| c.set_context(used, window, cx));
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
        let shown = match self.density {
            ToolDensity::Grouped => list_diff::grouped(items, &|ix| crate::session_view::calls::shows(items, ix), &self.reviews.turn_marks),
            ToolDensity::Lines | ToolDensity::Detailed => list_diff::rows(items.len(), &self.reviews.turn_marks),
        };
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
        let now = std::time::Instant::now();
        self.arrived.retain(|_, at| now.duration_since(*at) < ARRIVAL_KEPT);
        for row in list_diff::arrivals(&self.shown, &shown) {
            self.arrived.insert(row, now);
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
        self.asked_turn = true;
        // Running from the moment it goes, not from the agent's first word: ⌘↵ before then queues.
        self.composer.update(cx, |c, cx| c.set_running(true, cx));
        self.status = status::sent();
        self.problem = None;
        self.stderr = None;
        self.refresh_rows();
        // A message sent goes to the end and the follow takes hold again.
        self.glide.follow();
        // The review comments go with the message, and show resolved once the agent's turn ends.
        let attachments = self.reviews.send_comments();
        if !attachments.is_empty() {
            self.save_review(cx);
        }
        if let Some(command) = self.with_brief(Command::Send { text, attachments }, cx) {
            self.dispatch(command, cx);
        }
        cx.emit(SessionEvent::Changed);
    }

    /// Sends `command`, which starts a turn: now, or once the agent is running again.
    pub(super) fn dispatch(&mut self, command: Command, cx: &mut Context<Self>) {
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

    /// Runs the session on `choice` from here: the agent starts again on it. Only before the first message.
    pub fn set_provider(&mut self, choice: crate::providers::Choice, cx: &mut Context<Self>) {
        if !self.can_choose_agent() || self.provider.as_ref() == Some(&choice) {
            return;
        }
        self.provider = Some(choice);
        self.restart(cx);
    }

    /// Drops the agent and starts a new session in its place, as the provider or a handoff now says.
    pub(super) fn restart(&mut self, cx: &mut Context<Self>) {
        self.session = None;
        self.problem = None;
        self.status = SessionStatus::Idle;
        self.starting = true;
        self.open(None, false, cx);
        cx.emit(SessionEvent::Changed);
        cx.notify();
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

    fn show_card(&mut self, reference: atelier_forge::PullRef, cx: &mut Context<Self>) {
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
    fn draft_title(&mut self, cx: &mut Context<Self>) {
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
