//! Tasks in the app: the tracker of a project, what beui shows of it, and the pane that holds them.
//! See plans/m6-tasks.md.
use std::sync::Arc;

use gpui_kit::{AppContext, Context, Entity, SharedString, Subscription, Window};
use atelier_project::Project;

use crate::open_project::OpenProject;

pub mod map;
pub mod pane;
pub mod signal;
#[cfg(test)]
mod tests;

use pane::{TasksEvent, TasksPane};

/// The task a session began from: its id, and the short key the header shows.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskRef {
    pub id: atelier_tracker::TaskId,
    pub key: SharedString,
}

/// The rules the reader left on. Reads the settings file, so call it off the UI thread.
pub fn rules() -> atelier_tracker::RuleSet {
    let off = atelier_settings::path().map(|path| atelier_settings::load(&path).task_rules_off).unwrap_or_default();
    atelier_tracker::RuleSet::from_disabled(off.iter().map(String::as_str))
}

/// A project's tasks, once asked for: the pane, and whether it is in the right pane now.
pub struct Slot {
    pub pane: Entity<TasksPane>,
    pub shown: bool,
    _events: Subscription,
}

impl Slot {
    /// The pane for `project`; its tracker opens off the UI thread.
    pub fn new(project: Arc<dyn Project>, window: &mut Window, cx: &mut Context<OpenProject>) -> Self {
        let me = std::env::var("USER").unwrap_or_else(|_| "me".to_string());
        let agents = vec![("Claude".into(), atelier_agents::claude::look())];
        let pane = cx.new(|cx| {
            let mut pane = TasksPane::new(me, agents, window, cx);
            pane.open_from(project, cx);
            pane
        });
        let _events = cx.subscribe_in(&pane, window, |this: &mut OpenProject, _, event: &TasksEvent, window, cx| match event {
            TasksEvent::Start(id) => this.start_from_task(id.clone(), window, cx),
            TasksEvent::Counted(open) => {
                this.set_task_count(*open, cx);
            }
        });
        Self { pane, shown: true, _events }
    }
}
