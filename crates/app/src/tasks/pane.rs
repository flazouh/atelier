//! The Tasks pane: a project's tasks as a list or a board, one task in full, and the create dialog. It takes
//! the right pane, as the pull requests do. It reads and writes the project's tracker off the UI thread
//! and keeps its own copy of the tasks, which the four parts share: a change in one shows in the others.
use std::sync::Arc;

use beui::{
    ActiveTheme, AgentLook, Button, ButtonSize, ButtonVariant, Modal, Segment, Segmented,
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
    AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Window, canvas, div, prelude::FluentBuilder, px,
};
use lathe_tracker::{Entry, Patch, Query, Task, TaskId, Tracker, TrackerResult};

use super::map;

/// Under this width the board would clip, so the pane shows the list and hides the switch.
pub const BOARD_LEAST: f32 = 560.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    List,
    Board,
}

enum Load {
    Loading,
    Ready,
    Failed(SharedString),
}

/// What the pane asks of the app.
pub enum TasksEvent {
    /// The reader asked for a session for this task.
    Start(TaskId),
}

pub struct TasksPane {
    tracker: Result<Arc<dyn Tracker>, SharedString>,
    /// Hears of changes made elsewhere while the pane lives.
    _watch: Option<gpui_kit::Task<()>>,
    me: SharedString,
    agents: Vec<(SharedString, AgentLook)>,
    people: Vec<Assignee>,
    labels: Vec<Label>,
    tasks: Vec<TaskData>,
    load: Load,
    mode: Mode,
    open: Option<SharedString>,
    creating: bool,
    said: Option<SharedString>,
    width: f32,
    list: Entity<TaskList>,
    board: Entity<TaskBoard>,
    view: Entity<TaskView>,
    dialog: Entity<NewTask>,
    focus: FocusHandle,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    List,
    Board,
    View,
    None,
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
            TaskViewEvent::StartSession(id) => cx.emit(TasksEvent::Start(TaskId(id.to_string()))),
            TaskViewEvent::OpenSession(_) | TaskViewEvent::OpenPr(_) => {}
        }));
        subscriptions.push(cx.subscribe_in(&dialog, window, |this: &mut Self, _, event: &NewTaskEvent, window, cx| match event {
            NewTaskEvent::Cancel => this.close_dialog(window, cx),
            NewTaskEvent::Create { draft, start_session } => this.create(draft, *start_session, window, cx),
        }));
        Self {
            tracker: Err(SharedString::default()),
            _watch: None,
            me,
            agents,
            people,
            labels: Vec::new(),
            tasks: Vec::new(),
            load: Load::Loading,
            mode: Mode::List,
            open: None,
            creating: false,
            said: None,
            width: 0.,
            list,
            board,
            view,
            dialog,
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    /// Opens the project's tracker, off the UI thread (a project over SSH asks its host), and shows the
    /// tasks when it is open.
    pub fn open_from(&mut self, project: Arc<dyn lathe_project::Project>, cx: &mut Context<Self>) {
        let opening = cx.background_spawn(async move { project.tracker() });
        cx.spawn(async move |this, cx| {
            let opened = opening.await.map_err(|error| SharedString::from(error.to_string()));
            this.update(cx, |pane, cx| pane.attach(opened, cx)).ok();
        })
        .detach();
    }

    /// Gives the pane its tracker, or the words that say why it has none.
    pub fn attach(&mut self, tracker: Result<Arc<dyn Tracker>, SharedString>, cx: &mut Context<Self>) {
        match &tracker {
            Ok(tracker) => {
                // The watch wakes the pane from its own thread, which the test scheduler rejects; the
                // subscription itself is tested in lathe-tracker and lathe-remote.
                if !cfg!(test) {
                    self._watch = Some(Self::watch(tracker.clone(), cx));
                }
                self.tracker = Ok(tracker.clone());
                self.reload(cx);
            }
            Err(why) => {
                self.load = Load::Failed(why.clone());
                self.tracker = Err(why.clone());
                cx.notify();
            }
        }
    }

    /// Reads the tasks again when a change is made elsewhere, for as long as the pane lives. The
    /// subscription is dropped with the pane, and that stops a remote poll.
    fn watch(tracker: Arc<dyn Tracker>, cx: &mut Context<Self>) -> gpui_kit::Task<()> {
        use futures_util::StreamExt as _;
        let subscription = tracker.subscribe();
        let (told, mut heard) = futures_channel::mpsc::unbounded::<()>();
        std::thread::spawn(move || {
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

    /// The tracker the pane reads and writes, when the project has one.
    pub fn tracker(&self) -> Option<Arc<dyn Tracker>> {
        self.tracker.as_ref().ok().cloned()
    }

    #[cfg(test)]
    pub fn tasks(&self) -> &[TaskData] {
        &self.tasks
    }

    /// The board needs room; a narrow pane shows the list whatever was asked.
    pub fn shown_mode(&self) -> Mode {
        if self.width > 0. && self.width < BOARD_LEAST { Mode::List } else { self.mode }
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

    /// Reads every task and the labels again, off the UI thread.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let Ok(tracker) = self.tracker.clone() else { return };
        let reading = cx.background_spawn(async move {
            let tasks = tracker.list(&Query::default())?;
            let labels = tracker.labels()?;
            TrackerResult::Ok((tasks, labels))
        });
        cx.spawn(async move |this, cx| {
            let result = reading.await;
            this.update(cx, |this, cx| this.loaded(result, cx)).ok();
        })
        .detach();
    }

    fn loaded(&mut self, result: TrackerResult<(Vec<Task>, Vec<String>)>, cx: &mut Context<Self>) {
        match result {
            Ok((tasks, labels)) => {
                let looks = self.looks(cx);
                let kept: std::collections::HashMap<SharedString, Vec<beui::task_model::Activity>> =
                    self.tasks.iter().map(|t| (t.id.clone(), t.activity.clone())).collect();
                self.tasks = tasks
                    .iter()
                    .map(|t| {
                        let mut data = map::task_data(t, &[], &looks);
                        data.activity = kept.get(&data.id).cloned().unwrap_or_default();
                        data
                    })
                    .collect();
                self.labels = labels.iter().map(|l| map::label_of(l)).collect();
                self.load = Load::Ready;
                let (people, labels) = (self.people.clone(), self.labels.clone());
                self.dialog.update(cx, |d, _| d.set_people(people, labels));
                self.push_all(Source::None, cx);
            }
            Err(error) => self.load = Load::Failed(error.to_string().into()),
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

    fn focus_body(&self, window: &mut Window, cx: &mut Context<Self>) {
        match (&self.open, self.shown_mode()) {
            (Some(_), _) => window.focus(&self.view.focus_handle(cx), cx),
            (None, Mode::List) => window.focus(&self.list.focus_handle(cx), cx),
            (None, Mode::Board) => window.focus(&self.board.focus_handle(cx), cx),
        }
    }

    /// Shows one task in full, with its activity, which is read first.
    pub fn show(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.open = Some(id.clone());
        self.creating = false;
        self.push_all(Source::None, cx);
        self.focus_body(window, cx);
        cx.notify();
        let Ok(tracker) = self.tracker.clone() else { return };
        let task_id = TaskId(id.to_string());
        let reading = cx.background_spawn(async move { tracker.activity(&task_id) });
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

    /// Back from one task to the list or the board.
    pub fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = None;
        self.focus_body(window, cx);
        cx.notify();
    }

    fn new_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.creating = true;
        self.dialog.update(cx, |d, cx| d.reset(window, cx));
        cx.notify();
    }

    fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.creating = false;
        self.focus_body(window, cx);
        cx.notify();
    }

    fn create(&mut self, draft: &beui::new_task_model::Draft, start_session: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.close_dialog(window, cx);
        let (Ok(tracker), Some(new)) = (self.tracker.clone(), Some(map::new_task_of(draft))) else { return };
        let by = self.me.to_string();
        let making = cx.background_spawn(async move { tracker.create(&new, &by) });
        cx.spawn(async move |this, cx| {
            let made = making.await;
            this.update(cx, |this, cx| match made {
                Ok(task) => {
                    this.reload(cx);
                    if start_session {
                        cx.emit(TasksEvent::Start(task.id));
                    }
                }
                Err(error) => this.say(format!("Could not make the task: {error}"), cx),
            })
            .ok();
        })
        .detach();
    }

    fn say(&mut self, words: String, cx: &mut Context<Self>) {
        self.said = Some(words.into());
        cx.notify();
    }

    /// A change the reader made in one of the parts: kept in the tracker, and shown in the others.
    fn changed(&mut self, ids: &[SharedString], change: &Change, source: Source, cx: &mut Context<Self>) {
        let patches = map::patches_of(&self.tasks, ids, change);
        task_edit::apply(&mut self.tasks, ids, change, &self.me, crate::agent_session::now());
        self.push_all(source, cx);
        self.save(patches, cx);
        cx.notify();
    }

    fn save(&mut self, patches: Vec<(TaskId, Patch)>, cx: &mut Context<Self>) {
        let Ok(tracker) = self.tracker.clone() else { return };
        let by = self.me.to_string();
        let saving = cx.background_spawn(async move {
            patches.iter().try_for_each(|(id, patch)| tracker.update(id, patch, &by).map(drop))
        });
        cx.spawn(async move |this, cx| {
            let saved = saving.await;
            this.update(cx, |this, cx| {
                // A failed save puts what the tracker holds back, and says so.
                if let Err(error) = saved {
                    this.say(format!("Could not save: {error}"), cx);
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
        let patch = Patch { description: Some(text.to_string()), ..Patch::default() };
        self.save(vec![(TaskId(id.to_string()), patch)], cx);
    }

    fn comment(&mut self, id: &SharedString, text: &SharedString, cx: &mut Context<Self>) {
        let Ok(tracker) = self.tracker.clone() else { return };
        let (task_id, entry, by) = (TaskId(id.to_string()), Entry::Comment(text.to_string()), self.me.to_string());
        let writing = cx.background_spawn(async move { tracker.record(&task_id, &entry, &by) });
        cx.spawn(async move |this, cx| {
            if let Err(error) = writing.await {
                this.update(cx, |this, cx| this.say(format!("Could not save the comment: {error}"), cx)).ok();
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
        let header = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .h(px(44.))
            .px(px(12.))
            .child(div().text_size(TextSize::Sm.font_size()).font_weight(gpui_kit::FontWeight::MEDIUM).child("Tasks"))
            .children(switch)
            .child(div().flex_1())
            .when(self.open.is_some(), |d| {
                let pane = pane.clone();
                d.child(
                    Button::new("tasks-back")
                        .label("All tasks")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .cap("Esc")
                        .on_click(move |_, window, cx| pane.update(cx, |p, cx| p.back(window, cx))),
                )
            })
            .child(
                Button::new("tasks-new")
                    .label("New task")
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Sm)
                    .cap("c")
                    .on_click(move |_, window, cx| pane.update(cx, |p, cx| p.new_task(window, cx))),
            );
        let body = match (&self.load, &self.open, mode) {
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
                .width(600.)
                .focus(&focus)
                .on_close(move |window, cx| pane.update(cx, |p, cx| p.close_dialog(window, cx)))
                .child(self.dialog.clone())
        });
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
            .rounded(radius::LG)
            .bg(theme.card)
            .child(measure)
            .child(header)
            .when_some(self.said.clone(), |d, said| {
                d.child(div().px(px(12.)).pb(px(6.)).text_size(TextSize::Xs.font_size()).text_color(theme.danger).child(said))
            })
            .child(div().flex_1().min_h_0().child(body))
            .children(dialog)
    }
}

#[cfg(test)]
mod tests;
