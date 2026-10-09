use std::{collections::HashMap, sync::Arc};

use atelier_capabilities::{
    Actor, CapError, CapResult, Operation, Ref,
    tasks::{self as v1, TasksProvider},
};
use atelier_ui::{
    ActiveTheme,
    AgentLook,
    Button,
    ButtonSize,
    ButtonVariant,
    Modal,
    Segment,
    Segmented,
    new_task::{NewTask, NewTaskEvent},
    task_board::{TaskBoard, TaskBoardEvent},
    task_edit::{self, Change},
    task_list::{TaskList, TaskListEvent},
    task_model::{Assignee, Label, TaskData},
    task_view::{TaskView, TaskViewEvent},
    theme::radius,
    typography::TextSize,
};
use gpui_kit::{
    AppContext,
    Context,
    Entity,
    EventEmitter,
    FocusHandle,
    Focusable,
    InteractiveElement,
    IntoElement,
    ParentElement,
    Render,
    SharedString,
    Styled,
    Subscription,
    Window,
    canvas,
    div,
    prelude::FluentBuilder,
};
use atelier_ui::scale::px;
use atelier_tracker::Tracker;

use super::super::{
    map,
    source::{TasksSource, Vocabulary},
};
use super::helpers::{apply, banner, load_more, react, read, read_activity, read_more, signed_out, switcher};
use super::types::{BOARD_LEAST, Load, Mode, Problem, Reaction, Scope, Source, TasksEvent};

/// What one reading of a provider brings.
pub struct Loaded {
    pub tasks: Vec<v1::Task>,
    /// The cursor of the next page, when there is one.
    pub next: Option<String>,
    pub vocab: Vocabulary,
}

pub struct TasksPane {
    pub(super) source: Result<TasksSource, SharedString>,
    /// What the shown provider offers and names; it decides which controls show.
    pub(super) vocab: Vocabulary,
    /// Hears of changes made elsewhere while the pane lives.
    _watch: Option<gpui_kit::Task<()>>,
    pub(super) me: SharedString,
    agents: Vec<(SharedString, AgentLook)>,
    people: Vec<Assignee>,
    pub(super) labels: Vec<Label>,
    pub(super) tasks: Vec<TaskData>,
    /// The key of a task to show in full as soon as the tasks are read.
    show_when_read: Option<SharedString>,
    /// The version of each task as last read, by reference, which an update sends back.
    versions: HashMap<String, String>,
    /// The cursor of the page after the last one read.
    pub(super) next: Option<String>,
    /// How many pages are read, so a reading after a change keeps what the reader loaded.
    pages: usize,
    loading_more: bool,
    /// Counts the provider switches, so a reading that comes after one is dropped.
    generation: u64,
    pub(super) problem: Option<Problem>,
    /// The line in `said` is about reading the tasks, so a good reading clears it.
    said_reading: bool,
    pub(super) load: Load,
    pub(super) mode: Mode,
    scope: Scope,
    pub(super) open: Option<SharedString>,
    pub(super) creating: bool,
    said: Option<SharedString>,
    /// The task to put the cursor on when the tasks are read again: the one just made.
    select_after: Option<SharedString>,
    pub(super) width: f32,
    pub(super) list: Entity<TaskList>,
    pub(super) board: Entity<TaskBoard>,
    view: Entity<TaskView>,
    pub(super) dialog: Entity<NewTask>,
    pub(super) focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<TasksEvent> for TasksPane {}

impl Focusable for TasksPane {
    /// The part the keys go to: the task in full, the board, or the list.
    fn focus_handle(&self, cx: &gpui_kit::App) -> FocusHandle {
        match (&self.open, self.shown_mode()) {
            (Some(_), _) => self.view.focus_handle(cx),
            (None, Mode::List) => self.list.focus_handle(cx),
            (None, Mode::Board) => self.board.focus_handle(cx),
        }
    }
}

impl TasksPane {
    /// The pane, before its tracker: see [`Self::open_from`]. `agents` are the agents that can be
    /// assigned, with their looks.
    pub fn new(
        me: impl Into<SharedString>,
        agents: Vec<(SharedString, AgentLook)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let me: SharedString = me.into();
        let mut people = vec![Assignee::Person { name: me.clone() }];
        people.extend(agents.iter().map(|(name, look)| Assignee::agent(name.clone(), look.clone())));
        let list = cx.new(|cx| TaskList::new(me.clone(), cx));
        let board = cx.new(|cx| {
            let mut board = TaskBoard::new(me.clone(), cx);
            board.set_people(people.clone());
            board
        });
        let view = cx.new(|cx| TaskView::new(me.clone(), window, cx));
        let dialog = cx.new(|cx| NewTask::new(people.clone(), Vec::new(), window, cx));
        let mut subscriptions = Vec::new();
        subscriptions.push(cx.subscribe_in(&list, window, |this: &mut Self, _, event: &TaskListEvent, window, cx| match event {
            TaskListEvent::Open(id) => this.show(id.clone(), window, cx),
            TaskListEvent::Changed { ids, change } => this.changed(ids, change, Source::List, cx),
            TaskListEvent::NewTask => this.new_task(window, cx),
        }));
        subscriptions.push(cx.subscribe_in(&board, window, |this: &mut Self, _, event: &TaskBoardEvent, window, cx| match event {
            TaskBoardEvent::Open(id) => this.show(id.clone(), window, cx),
            TaskBoardEvent::Changed { ids, change } => this.changed(ids, change, Source::Board, cx),
            TaskBoardEvent::NewTask => this.new_task(window, cx),
        }));
        subscriptions.push(cx.subscribe_in(&view, window, |this: &mut Self, _, event: &TaskViewEvent, window, cx| match event {
            TaskViewEvent::Changed { id, change } => this.changed(std::slice::from_ref(id), change, Source::View, cx),
            TaskViewEvent::OpenTask(id) => this.show(id.clone(), window, cx),
            TaskViewEvent::DescriptionSaved { id, text } => this.describe(id, text, cx),
            TaskViewEvent::Commented { id, text } => this.comment(id, text, cx),
            TaskViewEvent::StartSession(id) => {
                if let Ok(task) = id.parse::<Ref>() {
                    cx.emit(TasksEvent::Start(task));
                }
            }
            TaskViewEvent::OpenSession(_) | TaskViewEvent::OpenPr(_) => {}
        }));
        subscriptions.push(cx.subscribe_in(&dialog, window, |this: &mut Self, _, event: &NewTaskEvent, window, cx| match event {
            NewTaskEvent::Cancel => this.close_dialog(window, cx),
            NewTaskEvent::Create { draft, start_session } => this.create(draft, *start_session, window, cx),
        }));
        Self {
            source: Err(SharedString::default()),
            vocab: Vocabulary::default(),
            _watch: None,
            me,
            agents,
            people,
            labels: Vec::new(),
            tasks: Vec::new(),
            show_when_read: None,
            versions: HashMap::new(),
            next: None,
            pages: 1,
            loading_more: false,
            generation: 0,
            problem: None,
            said_reading: false,
            load: Load::Loading,
            mode: Mode::List,
            scope: Scope::All,
            open: None,
            creating: false,
            said: None,
            select_after: None,
            width: 0.,
            list,
            board,
            view,
            dialog,
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    /// Opens the project's providers, off the UI thread (a project over SSH asks its host), and shows the
    /// tasks when they are open.
    pub fn open_from(&mut self, project: Arc<dyn atelier_project::Project>, cx: &mut Context<Self>) {
        let me = self.me.to_string();
        let opening = cx.background_spawn(async move { TasksSource::open(&project, &me) });
        cx.spawn(async move |this, cx| {
            let opened = opening.await.map_err(SharedString::from);
            this.update(cx, |pane, cx| pane.attach(opened, cx)).ok();
        })
        .detach();
    }

    /// Gives the pane its providers, or the words that say why it has none.
    pub fn attach(&mut self, source: Result<TasksSource, SharedString>, cx: &mut Context<Self>) {
        match source {
            Ok(source) => {
                self.source = Ok(source);
                self.follow(cx);
                self.reload(cx);
            }
            Err(why) => {
                self.load = Load::Failed(why.clone());
                self.source = Err(why);
                cx.notify();
            }
        }
    }

    /// Follows the shown provider, so a change made elsewhere shows.
    fn follow(&mut self, cx: &mut Context<Self>) {
        // The watch wakes the pane from its own thread, which the test scheduler rejects; the
        // subscription itself is tested in atelier-tracker and atelier-remote.
        self._watch = if cfg!(test) { None } else { self.provider().map(|provider| Self::watch(provider, cx)) };
    }

    /// Reads the tasks again when a change is made elsewhere, for as long as the pane lives. The
    /// subscription is dropped with the pane, and that stops a remote poll. It is asked for off the UI thread.
    fn watch(provider: Arc<dyn TasksProvider>, cx: &mut Context<Self>) -> gpui_kit::Task<()> {
        use futures_util::StreamExt as _;
        let (told, mut heard) = futures_channel::mpsc::unbounded::<()>();
        std::thread::spawn(move || {
            let Ok(subscription) = provider.subscribe() else { return };
            while !told.is_closed() {
                match subscription.recv_timeout(std::time::Duration::from_millis(500)) {
                    Ok(_) => {
                        if told.unbounded_send(()).is_err() {
                            break;
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        cx.spawn(async move |this, cx| {
            while heard.next().await.is_some() {
                // A burst of changes is one reading.
                while heard.try_recv().is_ok() {}
                if this.update(cx, |pane, cx| pane.reload(cx)).is_err() {
                    break;
                }
            }
        })
    }

    /// The provider the pane reads and writes, when the project has one.
    pub fn provider(&self) -> Option<Arc<dyn TasksProvider>> {
        self.source.as_ref().ok()?.provider()
    }

    /// The project's local tracker, for the rules and the session and pull request links. They stay on the tracker
    /// until the rules move to the capability, so this is the local one whichever provider is shown.
    pub fn tracker(&self) -> Option<Arc<dyn Tracker>> {
        self.source.as_ref().ok()?.local()
    }

    /// Shows the provider at `index` of the switcher, and reads its tasks.
    pub fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        let Ok(source) = &mut self.source else { return };
        if source.selected() == Some(index) || !source.select(index) {
            return;
        }
        self.generation += 1;
        self.tasks.clear();
        self.labels.clear();
        self.versions.clear();
        self.vocab = Vocabulary::default();
        (self.next, self.pages, self.loading_more) = (None, 1, false);
        (self.problem, self.said, self.open, self.load) = (None, None, None, Load::Loading);
        self.push_all(Source::None, cx);
        self.follow(cx);
        self.reload(cx);
        cx.notify();
    }

    #[cfg(test)]
    pub fn tasks(&self) -> &[TaskData] {
        &self.tasks
    }

    /// The board needs room; a narrow pane shows the list whatever was asked.
    pub fn shown_mode(&self) -> Mode {
        if self.width > 0. && self.width < BOARD_LEAST { Mode::List } else { self.mode }
    }

    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    /// Shows the issues of `scope`, in the list and on the board, and closes the issue shown in full.
    pub fn set_scope(&mut self, scope: Scope, window: &mut Window, cx: &mut Context<Self>) {
        self.list.update(cx, |list, cx| {
            let f = scope.filters(list.filters());
            list.set_filters(f, cx);
        });
        self.board.update(cx, |board, cx| {
            let f = scope.filters(board.filters());
            board.set_filters(f, cx);
        });
        self.scope = scope;
        self.open = None;
        self.focus_body(window, cx);
        cx.notify();
    }

    /// How many issues `scope` holds.
    pub fn count(&self, scope: &Scope) -> usize {
        scope.count(&self.tasks, &self.me)
    }

    pub fn labels(&self) -> &[Label] {
        &self.labels
    }

    /// The agents that hold at least one issue.
    pub fn agents_at_work(&self) -> Vec<(SharedString, AgentLook)> {
        self.agents.iter().filter(|(name, _)| self.count(&Scope::Agent(name.clone())) > 0).cloned().collect()
    }

    pub fn set_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        self.mode = mode;
        self.open = None;
        self.focus_body(window, cx);
        cx.notify();
    }

    fn looks(&self, cx: &gpui_kit::App) -> impl Fn(&str) -> AgentLook + use<> {
        let agents = self.agents.clone();
        let neutral = AgentLook::neutral(cx.theme());
        move |name: &str| agents.iter().find(|(n, _)| n.as_ref() == name).map_or_else(|| neutral.clone(), |(_, look)| look.clone())
    }

    /// Reads the tasks, the statuses and the labels again, off the UI thread.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(provider) = self.provider() else { return };
        let (pages, generation) = (self.pages, self.generation);
        let reading = cx.background_spawn(async move { read(provider.as_ref(), pages) });
        cx.spawn(async move |this, cx| {
            let result = reading.await;
            this.update(cx, |this, cx| this.loaded(result, generation, cx)).ok();
        })
        .detach();
    }

    /// The rows for `tasks`, with the activity of the rows the pane already holds.
    fn rows_of(&self, tasks: &[v1::Task], cx: &gpui_kit::App) -> Vec<TaskData> {
        let looks = self.looks(cx);
        let kept: HashMap<SharedString, Vec<atelier_ui::task_model::Activity>> = self.tasks.iter().map(|t| (t.id.clone(), t.activity.clone())).collect();
        tasks
            .iter()
            .map(|t| {
                let mut data = map::task_data(t, &[], &self.vocab, &looks);
                data.activity = kept.get(&data.id).cloned().unwrap_or_default();
                data
            })
            .collect()
    }

    fn loaded(&mut self, result: CapResult<Loaded>, generation: u64, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        match result {
            Ok(Loaded { tasks, next, vocab }) => {
                self.vocab = vocab;
                self.tasks = self.rows_of(&tasks, cx);
                self.versions = tasks.iter().map(|t| (t.reference.to_string(), t.version.clone())).collect();
                self.next = next;
                // A provider without labels shows none, in the sidebar, the pickers and the dialog.
                self.labels = match self.vocab.can(Operation::Labels) {
                    true => self.vocab.labels().iter().map(|l| map::label_of(&l.name)).collect(),
                    false => Vec::new(),
                };
                self.load = Load::Ready;
                self.problem = None;
                if std::mem::take(&mut self.said_reading) {
                    self.said = None;
                }
                let (people, labels) = (self.people.clone(), self.labels.clone());
                self.dialog.update(cx, |d, _| d.set_people(people, labels));
                self.push_all(Source::None, cx);
                let asked = self.show_when_read.take();
                if let Some(id) = asked.and_then(|key| self.tasks.iter().find(|t| t.key == key).map(|t| t.id.clone())) {
                    self.open_task(id, cx);
                }
                // A key needs a task under the cursor: the one just made, else the first when there is none.
                let target = self.select_after.take();
                if target.is_some() || self.list.read(cx).cursor().row.is_none() {
                    self.list.update(cx, |list, cx| list.put_cursor_on(target.as_ref(), cx));
                }
                if target.is_some() || self.board.read(cx).cursor().is_none() {
                    self.board.update(cx, |board, cx| board.put_cursor_on(target.as_ref(), cx));
                }
            }
            Err(error) => self.fail(error, Operation::List, "Could not read the tasks", cx),
        }
        cx.notify();
    }

    /// Reads the page after the last one, when there is one, and adds it to the tasks.
    pub fn load_more(&mut self, cx: &mut Context<Self>) {
        let (Some(provider), Some(cursor)) = (self.provider(), self.next.clone()) else { return };
        if self.loading_more {
            return;
        }
        self.loading_more = true;
        let generation = self.generation;
        let reading = cx.background_spawn(async move { read_more(provider.as_ref(), &cursor) });
        cx.spawn(async move |this, cx| {
            let result = reading.await;
            this.update(cx, |this, cx| this.more(result, generation, cx)).ok();
        })
        .detach();
        cx.notify();
    }

    fn more(&mut self, result: CapResult<v1::Page<v1::Task>>, generation: u64, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        self.loading_more = false;
        match result {
            Ok(page) => {
                let fresh: Vec<v1::Task> = page.items.into_iter().filter(|t| !self.versions.contains_key(&t.reference.to_string())).collect();
                self.versions.extend(fresh.iter().map(|t| (t.reference.to_string(), t.version.clone())));
                let rows = self.rows_of(&fresh, cx);
                self.tasks.extend(rows);
                self.next = page.next_cursor;
                self.pages += 1;
                self.push_all(Source::None, cx);
            }
            Err(error) => self.fail(error, Operation::List, "Could not read more tasks", cx),
        }
        cx.notify();
    }

    /// Answers an error of a call as [`react`] says. `doing` starts the line that says it.
    pub(super) fn fail(&mut self, error: CapError, operation: Operation, doing: &str, cx: &mut Context<Self>) {
        match react(&error) {
            Reaction::Raise(problem) => {
                self.problem = Some(problem);
                // The banner sits over the screen, so a first reading that failed shows it over the empty list.
                if matches!(self.load, Load::Loading) {
                    self.load = Load::Ready;
                }
            }
            // The control of a call the provider does not offer goes. A list it cannot give is a plain failure.
            Reaction::Hide if operation != Operation::List => self.vocab.disable(operation),
            Reaction::Hide | Reaction::Line(_) => {
                let words = error.to_string();
                if matches!(self.load, Load::Loading) {
                    self.load = Load::Failed(words.into());
                } else {
                    self.said = Some(format!("{doing}: {words}").into());
                    self.said_reading = operation == Operation::List;
                }
            }
        }
        cx.notify();
    }

    /// Gives every part the tasks again, except the one the change came from: it already has them.
    fn push_all(&mut self, source: Source, cx: &mut Context<Self>) {
        let (tasks, people) = (self.tasks.clone(), self.people.clone());
        let now = crate::agent_session::now();
        if source != Source::List {
            self.list.update(cx, |l, cx| l.set_tasks(tasks.clone(), people.clone(), now, cx));
        }
        if source != Source::Board {
            self.board.update(cx, |b, cx| b.set_tasks(tasks.clone(), now, cx));
        }
        if source != Source::View
            && let Some(task) = self.open.as_ref().and_then(|id| self.tasks.iter().find(|t| t.id == *id)).cloned()
        {
            let labels = self.labels.clone();
            self.view.update(cx, |v, cx| v.show(task, tasks, people, labels, now, cx));
        }
    }

    pub(super) fn focus_body(&self, window: &mut Window, cx: &mut Context<Self>) {
        match (&self.open, self.shown_mode()) {
            (Some(_), _) => window.focus(&self.view.focus_handle(cx), cx),
            (None, Mode::List) => window.focus(&self.list.focus_handle(cx), cx),
            (None, Mode::Board) => window.focus(&self.board.focus_handle(cx), cx),
        }
    }

    /// Shows one task in full, with its activity, which is read first.
    pub fn show(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.open_task(id, cx);
        self.focus_body(window, cx);
    }

    fn open_task(&mut self, id: SharedString, cx: &mut Context<Self>) {
        self.open = Some(id.clone());
        self.creating = false;
        self.push_all(Source::None, cx);
        cx.notify();
        let (Some(provider), Ok(task)) = (self.provider(), id.parse::<Ref>()) else { return };
        let reading = cx.background_spawn(async move { read_activity(provider.as_ref(), &task) });
        cx.spawn(async move |this, cx| {
            let Ok(lines) = reading.await else { return };
            this.update(cx, |this, cx| {
                let activity = map::activity_data(&lines);
                if let Some(task) = this.tasks.iter_mut().find(|t| t.id == id) {
                    task.activity = activity;
                }
                this.push_all(Source::None, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Shows the task with this key. A pane that has not read its tasks yet shows it when it has.
    pub fn show_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        match self.tasks.iter().find(|t| t.key.as_ref() == key).map(|t| t.id.clone()) {
            Some(id) => self.show(id, window, cx),
            None => self.show_when_read = Some(key.into()),
        }
    }

    /// Back from one task to the list or the board.
    pub fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = None;
        self.focus_body(window, cx);
        cx.notify();
    }

    pub(super) fn new_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A provider that does not list `create` has no way to make a task, from the button or from the key.
        if !self.vocab.can(Operation::Create) {
            return;
        }
        self.creating = true;
        self.dialog.update(cx, |d, cx| d.reset(window, cx));
        cx.notify();
    }

    fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.creating = false;
        self.focus_body(window, cx);
        cx.notify();
    }

    /// The person using the app, as the actor of a change.
    fn actor(&self) -> Actor {
        Actor::person(self.me.to_string(), self.me.to_string())
    }

    pub(super) fn create(&mut self, draft: &atelier_ui::new_task_model::Draft, start_session: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.close_dialog(window, cx);
        let Some(provider) = self.provider().filter(|_| self.vocab.can(Operation::Create)) else { return };
        let new = match map::new_task_of(draft, &self.vocab) {
            Ok(new) => new,
            Err(error) => return self.say(format!("Could not make the task: {error}"), cx),
        };
        let by = self.actor();
        let making = cx.background_spawn(async move { provider.create(&new, &by) });
        cx.spawn(async move |this, cx| {
            let made = making.await;
            this.update(cx, |this, cx| match made {
                Ok(task) => {
                    this.select_after = Some(task.reference.to_string().into());
                    this.reload(cx);
                    if start_session {
                        cx.emit(TasksEvent::Start(task.reference));
                    }
                }
                Err(error) => this.fail(error, Operation::Create, "Could not make the task", cx),
            })
            .ok();
        })
        .detach();
    }

    fn say(&mut self, words: String, cx: &mut Context<Self>) {
        self.said = Some(words.into());
        self.said_reading = false;
        cx.notify();
    }

    /// A change the reader made in one of the parts: kept in the provider, and shown in the others.
    pub(super) fn changed(&mut self, ids: &[SharedString], change: &Change, source: Source, cx: &mut Context<Self>) {
        // A change the provider cannot take is not made: the parts show the tasks as they were.
        let refusal = if !self.vocab.can(Operation::Update) {
            Some("This provider does not let you change tasks.".to_string())
        } else if let Change::Status(status) = change
            && self.vocab.status_id(*status).is_none()
        {
            Some(format!("This provider has no {} status.", status.words()))
        } else {
            None
        };
        if let Some(words) = refusal {
            self.push_all(Source::None, cx);
            return self.say(words, cx);
        }
        let patches = map::patches_of(&self.tasks, ids, change, &self.vocab);
        task_edit::apply(&mut self.tasks, ids, change, &self.me, crate::agent_session::now());
        self.push_all(source, cx);
        self.save(patches, cx);
        cx.notify();
    }

    fn save(&mut self, patches: Vec<(Ref, v1::Patch)>, cx: &mut Context<Self>) {
        let Some(provider) = self.provider() else { return };
        let by = self.actor();
        let work: Vec<(Ref, v1::Patch, String)> = patches
            .into_iter()
            .map(|(task, patch)| {
                let version = self.versions.get(&task.to_string()).cloned().unwrap_or_default();
                (task, patch, version)
            })
            .collect();
        let saving = cx.background_spawn(async move {
            work.iter().map(|(task, patch, version)| apply(provider.as_ref(), task, patch, version, &by).map(|made| (made.reference.to_string(), made.version))).collect::<CapResult<Vec<_>>>()
        });
        cx.spawn(async move |this, cx| {
            let saved = saving.await;
            this.update(cx, |this, cx| match saved {
                Ok(versions) => this.versions.extend(versions),
                // A failed save puts what the provider holds back, and says so.
                Err(error) => {
                    this.fail(error, Operation::Update, "Could not save", cx);
                    this.reload(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn describe(&mut self, id: &SharedString, text: &SharedString, cx: &mut Context<Self>) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == *id) {
            task.description = text.clone();
        }
        let Ok(task) = id.parse::<Ref>() else { return };
        let patch = v1::Patch { description: v1::Change::Set(text.to_string()), ..v1::Patch::default() };
        self.save(vec![(task, patch)], cx);
    }

    fn comment(&mut self, id: &SharedString, text: &SharedString, cx: &mut Context<Self>) {
        let (Some(provider), Ok(task)) = (self.provider(), id.parse::<Ref>()) else { return };
        if !self.vocab.can(Operation::Comment) {
            return self.say("This provider does not take comments.".into(), cx);
        }
        let (text, by) = (text.to_string(), self.actor());
        let writing = cx.background_spawn(async move { provider.comment(&task, &text, &by) });
        cx.spawn(async move |this, cx| {
            if let Err(error) = writing.await {
                this.update(cx, |this, cx| this.fail(error, Operation::Comment, "Could not save the comment", cx)).ok();
            }
        })
        .detach();
    }
}

impl Render for TasksPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let this = cx.entity().downgrade();
        let measure = canvas(
            move |bounds, _, cx| {
                let width = f32::from(bounds.size.width);
                this.update(cx, |pane, cx| {
                    if (pane.width - width).abs() > 0.5 {
                        pane.width = width;
                        cx.notify();
                    }
                })
                .ok();
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        let mode = self.shown_mode();
        let switch = (self.width == 0. || self.width >= BOARD_LEAST).then(|| {
            let pane = cx.entity();
            Segmented::new(
                "tasks-mode",
                [Segment::new("List").debug_name("tasks-mode-list"), Segment::new("Board").debug_name("tasks-mode-board")],
                if self.mode == Mode::List { 0 } else { 1 },
            )
            .on_change(move |i, window, cx| {
                pane.update(cx, |pane, cx| pane.set_mode(if i == 0 { Mode::List } else { Mode::Board }, window, cx))
            })
        });
        let pane = cx.entity();
        let choices = self.source.as_ref().ok().map(|s| (s.choices(), s.selected())).filter(|(choices, _)| choices.len() > 1);
        let provider_name = self.source.as_ref().ok().and_then(|s| s.selected().and_then(|i| s.choices().into_iter().nth(i))).map(|c| c.name());
        let switch_provider = choices.map(|(choices, selected)| {
            let pane = pane.clone();
            switcher(&choices, selected, move |i, _, cx| pane.update(cx, |p, cx| p.choose(i, cx)))
        });
        let header = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .h(px(44.))
            .px(px(12.))
            .child(div().text_size(TextSize::Sm.font_size()).font_weight(gpui_kit::FontWeight::MEDIUM).child(self.scope.title()))
            .children(switch)
            .children(switch_provider)
            .child(div().flex_1())
            .when(self.open.is_some(), |d| {
                let pane = pane.clone();
                d.child(
                    Button::new("tasks-back")
                        .label("Back")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .cap("Esc")
                        .on_click(move |_, window, cx| pane.update(cx, |p, cx| p.back(window, cx))),
                )
            })
            .when(self.vocab.can(Operation::Create), |d| {
                d.child(
                    Button::new("tasks-new")
                        .label("New task")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Sm)
                        .cap("c")
                        .on_click(move |_, window, cx| pane.update(cx, |p, cx| p.new_task(window, cx))),
                )
            });
        let body = match (&self.load, &self.open, mode) {
            (Load::Ready, ..) if self.problem == Some(Problem::SignedOut) => {
                let pane = cx.entity();
                signed_out(provider_name.as_deref().unwrap_or("this provider"), move |_, cx| pane.update(cx, |_, cx| cx.emit(TasksEvent::OpenSettings)), &theme)
            }
            (Load::Loading, ..) => div().flex_1().flex().items_center().justify_center().text_color(muted).child("Reading the tasks…").into_any_element(),
            (Load::Failed(why), ..) => {
                div().flex_1().flex().items_center().justify_center().px(px(24.)).text_color(muted).child(why.clone()).into_any_element()
            }
            (Load::Ready, Some(_), _) => self.view.clone().into_any_element(),
            (Load::Ready, None, Mode::List) => self.list.clone().into_any_element(),
            (Load::Ready, None, Mode::Board) => self.board.clone().into_any_element(),
        };
        let dialog = self.creating.then(|| {
            let pane = cx.entity();
            let focus = self.dialog.read(cx).focus_handle(cx);
            Modal::new("new-task-modal")
                .width(560.)
                .focus(&focus)
                .on_close(move |_, cx| pane.update(cx, |p, cx| p.dialog.update(cx, |d, cx| d.ask_cancel(cx))))
                .child(self.dialog.clone())
        });
        let notice = self.problem.and_then(|problem| {
            let pane = cx.entity();
            banner(problem, move |_, cx| pane.update(cx, |p, cx| p.reload(cx)), &theme)
        });
        // The next page is read from the list or the board; a task in full has no use for it.
        let more = self.next.is_some() && self.open.is_none() && matches!(self.load, Load::Ready) && self.problem != Some(Problem::SignedOut);
        let _ = window;
        div()
            .id("tasks-pane")
            .key_context("Tasks")
            .track_focus(&self.focus)
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .rounded(radius::lg())
            .bg(theme.card)
            .child(measure)
            .child(header)
            .when_some(self.said.clone(), |d, said| {
                d.child(div().px(px(12.)).pb(px(8.)).text_size(TextSize::Xs.font_size()).text_color(theme.danger).child(said))
            })
            .children(notice)
            .child(div().flex_1().min_h_0().child(body))
            .when(more, |d| {
                let pane = cx.entity();
                let loading = self.loading_more;
                d.child(load_more(loading, move |_, cx| pane.update(cx, |p, cx| p.load_more(cx))))
            })
            .children(dialog)
    }
}
