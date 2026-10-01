use atelier_ui::{
    ActiveTheme,
    AgentLook,
    Button,
    ButtonSize,
    ButtonVariant,
    new_task::{NewTask, NewTaskEvent},
    new_task_model::{create, next_key},
    task_board::{TaskBoard, TaskBoardEvent},
    task_edit::{self, Change},
    task_list::{TaskList, TaskListEvent},
    task_list_model::Filters,
    task_model::{Priority, TaskData},
    task_view::{TaskView, TaskViewEvent},
};
use gpui_kit::{
    AppContext,
    Context,
    Entity,
    Focusable,
    IntoElement,
    ParentElement,
    Render,
    SharedString,
    Styled,
    Subscription,
    Window,
    div,
    px,
};
use atelier_agents::claude;

use super::fixture::{BASE, ME, labels, people, tasks};
use crate::sidebar_story::run::Run;
use super::types::{FRAMES_OF_RUN, Source, TABS, Tab};

pub struct TasksStory {
    pub(super) tasks: Vec<TaskData>,
    pub(super) tab: Tab,
    pub(super) list: Entity<TaskList>,
    pub(super) board: Entity<TaskBoard>,
    pub(super) view: Entity<TaskView>,
    pub(super) dialog: Entity<NewTask>,
    pub(super) people: Vec<atelier_ui::task_model::Assignee>,
    pub(super) open: SharedString,
    pub(super) run: Option<Run>,
    pub(super) _subscriptions: Vec<Subscription>,
}

impl TasksStory {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let measuring = std::env::var("GALLERY_SCROLL").is_ok_and(|v| v == "1");
        let count = if measuring { std::env::var("TASK_COUNT").ok().and_then(|v| v.parse().ok()).unwrap_or(5000) } else { 200 };
        let claude = claude::look();
        let other = AgentLook::neutral(cx.theme());
        let people = people(&claude, &other);
        let data = tasks(count, &claude, &other);
        let list = cx.new(|cx| {
            let mut list = TaskList::new(ME, cx);
            list.set_tasks(data.clone(), people.clone(), BASE, cx);
            list
        });
        let board = cx.new(|cx| {
            let mut board = TaskBoard::new(ME, cx);
            board.set_people(people.clone());
            board.set_tasks(data.clone(), BASE, cx);
            board
        });
        let view = cx.new(|cx| TaskView::new(ME, window, cx));
        let dialog = cx.new(|cx| NewTask::new(people.clone(), labels(), window, cx));
        let open: SharedString = "t0".into();
        let mut subscriptions = Vec::new();
        subscriptions.push(cx.subscribe_in(&list, window, |this: &mut Self, _, event: &TaskListEvent, window, cx| match event {
            TaskListEvent::Open(id) => this.show(id.clone(), window, cx),
            TaskListEvent::Changed { ids, change } => this.changed(ids, change, Source::List, cx),
            TaskListEvent::NewTask => this.go(Tab::Create, window, cx),
        }));
        subscriptions.push(cx.subscribe_in(&board, window, |this: &mut Self, _, event: &TaskBoardEvent, window, cx| match event {
            TaskBoardEvent::Open(id) => this.show(id.clone(), window, cx),
            TaskBoardEvent::Changed { ids, change } => this.changed(ids, change, Source::Board, cx),
            TaskBoardEvent::NewTask => this.go(Tab::Create, window, cx),
        }));
        subscriptions.push(cx.subscribe_in(&view, window, |this: &mut Self, _, event: &TaskViewEvent, window, cx| match event {
            TaskViewEvent::Changed { id, change } => this.changed(std::slice::from_ref(id), change, Source::View, cx),
            TaskViewEvent::OpenTask(id) => this.show(id.clone(), window, cx),
            TaskViewEvent::DescriptionSaved { id, text } => {
                if let Some(task) = this.tasks.iter_mut().find(|t| t.id == *id) {
                    task.description = text.clone();
                }
            }
            _ => {}
        }));
        subscriptions.push(cx.subscribe_in(&dialog, window, |this: &mut Self, _, event: &NewTaskEvent, window, cx| match event {
            NewTaskEvent::Cancel => this.go(Tab::List, window, cx),
            NewTaskEvent::Create { draft, .. } => {
                let key = next_key("LAT", &this.tasks);
                let id = format!("t{}", this.tasks.len());
                if let Some(task) = create(draft, id, key, ME, BASE) {
                    this.tasks.push(task);
                    this.push_all(Source::None, cx);
                }
                this.go(Tab::List, window, cx);
            }
        }));
        let mut story = Self { tasks: data, tab: Tab::List, list, board, view, dialog, people, open, run: measuring.then(Run::new), _subscriptions: subscriptions };
        let start = match std::env::var("TASKS_VIEW").as_deref() {
            Ok("board") => Tab::Board,
            Ok("task") => Tab::Task,
            Ok("create") => Tab::Create,
            _ => Tab::List,
        };
        story.go(start, window, cx);
        story
    }

    pub(super) fn show(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.open = id;
        self.go(Tab::Task, window, cx);
    }

    pub(super) fn go(&mut self, tab: Tab, window: &mut Window, cx: &mut Context<Self>) {
        self.tab = tab;
        match tab {
            Tab::List => window.focus(&self.list.focus_handle(cx), cx),
            Tab::Board => window.focus(&self.board.focus_handle(cx), cx),
            Tab::Task => {
                if let Some(task) = self.tasks.iter().find(|t| t.id == self.open).cloned() {
                    let (all, people) = (self.tasks.clone(), self.people.clone());
                    self.view.update(cx, |v, cx| v.show(task, all, people, labels(), BASE, cx));
                }
                window.focus(&self.view.focus_handle(cx), cx);
            }
            Tab::Create => self.dialog.update(cx, |d, cx| d.reset(window, cx)),
        }
        cx.notify();
    }

    pub(super) fn changed(&mut self, ids: &[SharedString], change: &Change, source: Source, cx: &mut Context<Self>) {
        task_edit::apply(&mut self.tasks, ids, change, ME, BASE);
        self.push_all(source, cx);
    }

    /// Gives every view the tasks again, except the one the change came from: it already has them.
    pub(super) fn push_all(&mut self, source: Source, cx: &mut Context<Self>) {
        let (tasks, people) = (self.tasks.clone(), self.people.clone());
        if source != Source::List {
            self.list.update(cx, |l, cx| l.set_tasks(tasks.clone(), people.clone(), BASE, cx));
        }
        if source != Source::Board {
            self.board.update(cx, |b, cx| b.set_tasks(tasks.clone(), BASE, cx));
        }
        if source != Source::View
            && let Some(task) = self.tasks.iter().find(|t| t.id == self.open).cloned()
        {
            self.view.update(cx, |v, cx| v.show(task, tasks, people, labels(), BASE, cx));
        }
    }
}

impl Render for TasksStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        if let Some(run) = &mut self.run {
            match run.frame() {
                Some(n) => {
                    let total = FRAMES_OF_RUN;
                    // A filter for the middle half of the run; the frame that changes it is counted apart.
                    let want = if (total / 4..total * 3 / 4).contains(&n) { Some(Priority::Urgent) } else { None };
                    if self.tab == Tab::Board {
                        let has = self.board.read(cx).filters().priority;
                        if want != has {
                            run.switched();
                            self.board.update(cx, |b, cx| b.set_filters(Filters { priority: want, ..Filters::default() }, cx));
                        }
                        self.board.update(cx, |b, cx| {
                            b.set_scroll_top((n % 120) as f32 * 40.);
                            b.scroll_to((n % 60) as f32 * 30., cx);
                        });
                    } else {
                        let has = self.list.read(cx).filters().priority;
                        if want != has {
                            run.switched();
                            self.list.update(cx, |l, cx| l.set_filters(Filters { priority: want, ..Filters::default() }, cx));
                        }
                        self.list.update(cx, |l, _| l.set_scroll_top((n % 120) as f32 * 40.));
                    }
                }
                None => {
                    cx.quit();
                }
            }
            window.request_animation_frame();
        }
        let body = match self.tab {
            Tab::List => self.list.clone().into_any_element(),
            Tab::Board => self.board.clone().into_any_element(),
            Tab::Task => self.view.clone().into_any_element(),
            Tab::Create => {
                let this = cx.entity();
                let focus = self.dialog.read(cx).focus_handle(cx);
                atelier_ui::Modal::new("new-task-modal")
                    .width(560.)
                    .focus(&focus)
                    .on_close(move |window, cx| this.update(cx, |s, cx| s.go(Tab::List, window, cx)))
                    .child(self.dialog.clone())
                    .into_any_element()
            },        };
        let root = div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(12.))
            .child(div().flex().items_center().gap(px(6.)).children(TABS.iter().map(|&(tab, name)| {
                let this = cx.entity();
                Button::new(name)
                    .label(name)
                    .size(ButtonSize::Sm)
                    .variant(if tab == self.tab { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                    .on_click(move |_, window, cx| this.update(cx, |s, cx| s.go(tab, window, cx)))
            })))
            .child(div().flex_1().min_h_0().rounded(atelier_ui::theme::radius::xl()).bg(theme.card.opacity(0.35)).child(body));
        match &self.run {
            Some(run) => run.wrap(root.into_any_element()).into_any_element(),
            None => root.into_any_element(),
        }
    }
}
