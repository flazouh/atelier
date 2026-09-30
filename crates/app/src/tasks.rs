//! Tasks in the app: the tracker of a project, what beui shows of it, and the pane that holds them.
//! See plans/m6-tasks.md.
use std::sync::Arc;

use gpui_kit::{AppContext, Context, Entity, SharedString, Subscription, Window};
use lathe_tracker::Tracker;

use crate::open_project::OpenProject;

pub mod map;
pub mod pane;
pub mod signal;
pub mod store;
#[cfg(test)]
mod tests;

use pane::{TasksEvent, TasksPane};

/// The task a session began from: its id, and the short key the header shows.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskRef {
    pub id: lathe_tracker::TaskId,
    pub key: SharedString,
}

/// The rules the reader left on. Reads the settings file, so call it off the UI thread.
pub fn rules() -> lathe_tracker::RuleSet {
    let off = lathe_settings::path().map(|path| lathe_settings::load(&path).task_rules_off).unwrap_or_default();
    lathe_tracker::RuleSet::from_disabled(off.iter().map(String::as_str))
}

/// A project's tasks, once asked for: the pane, and whether it is in the right pane now.
pub struct Slot {
    pub pane: Entity<TasksPane>,
    pub shown: bool,
    _events: Subscription,
}

impl Slot {
    pub fn new(tracker: Result<Arc<dyn Tracker>, SharedString>, window: &mut Window, cx: &mut Context<OpenProject>) -> Self {
        let me = std::env::var("USER").unwrap_or_else(|_| "me".to_string());
        let agents = vec![("Claude".into(), lathe_agents::claude::look())];
        let pane = cx.new(|cx| TasksPane::new(tracker, me, agents, window, cx));
        let _events = cx.subscribe_in(&pane, window, |this: &mut OpenProject, _, event: &TasksEvent, window, cx| match event {
            TasksEvent::Start(id) => this.start_from_task(id.clone(), window, cx),
        });
        Self { pane, shown: true, _events }
    }
}
