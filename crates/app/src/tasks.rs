//! Tasks in the app: the tracker of a project, what beui shows of it, and the pane that holds them.
//! See plans/m6-tasks.md.
use std::sync::Arc;

use gpui_kit::{AppContext, Context, Entity, SharedString, Subscription, Window};
use lathe_tracker::Tracker;

use crate::open_project::OpenProject;

pub mod map;
pub mod pane;
pub mod store;
#[cfg(test)]
mod tests;

use pane::{TasksEvent, TasksPane};

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
        let _events = cx.subscribe_in(&pane, window, |_this: &mut OpenProject, _, event: &TasksEvent, _window, _cx| match event {
            // Started in the next step (M6a T2).
            TasksEvent::Start(id) => {
                let _ = id;
            }
        });
        Self { pane, shown: true, _events }
    }
}
