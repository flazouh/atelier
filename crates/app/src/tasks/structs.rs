use std::sync::Arc;

use gpui_kit::{AppContext, Context, Entity, SharedString, Subscription, Window};
use atelier_project::Project;

use crate::open_project::OpenProject;
use super::pane::{TasksEvent, TasksPane};

/// The task a session began from, in the local tracker: its id, and the short key the header shows. Session links are
/// local-only until they move to the capability.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskRef {
    pub id: atelier_tracker::TaskId,
    pub key: SharedString,
}

/// A project's tasks, once asked for: the pane, and whether it is in the right pane now.
pub struct Slot {
    pub pane: Entity<TasksPane>,
    pub shown: bool,
    _events: Subscription,
}

impl Slot {
    /// The pane for `project`; its providers open off the UI thread.
    pub fn new(project: Arc<dyn Project>, window: &mut Window, cx: &mut Context<OpenProject>) -> Self {
        let me = std::env::var("USER").unwrap_or_else(|_| "me".to_string());
        let agents = vec![("Claude".into(), atelier_agents::claude::look())];
        let pane = cx.new(|cx| {
            let mut pane = TasksPane::new(me, agents, window, cx);
            pane.open_from(project, cx);
            pane
        });
        let _events = cx.subscribe_in(&pane, window, |this: &mut OpenProject, _, event: &TasksEvent, window, cx| match event {
            TasksEvent::Start(task) => this.start_from_task(task.clone(), window, cx),
            TasksEvent::OpenAccounts => cx.emit(crate::open_project::ProjectEvent::OpenAccounts),
        });
        Self { pane, shown: true, _events }
    }
}
