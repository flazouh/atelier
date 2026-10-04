use gpui_kit::{App, Context, Entity, Window};
use atelier_agents::registry::Agent;

use crate::{agent_session::AgentSession, open_project::OpenProject};
use super::structs::Shell;

/// What the control socket (`control.rs`) asks of the window.
impl Shell {
    /// The projects open in this window.
    pub(crate) fn projects(&self) -> &[Entity<OpenProject>] {
        &self.projects
    }

    /// The session whose panel is in front.
    pub(crate) fn front_session(&self, cx: &App) -> Option<Entity<AgentSession>> {
        let key = self.panels.read(cx).active()?;
        self.session_by_key(key, cx).map(|(_, session)| session)
    }

    /// A new session of `agent` (the project's own with `None`) in the active project, in front.
    pub(crate) fn new_session_of(&mut self, agent: Option<Agent>, window: &mut Window, cx: &mut Context<Self>) -> Option<Entity<AgentSession>> {
        let at = self.active;
        let project = self.projects.get(at)?.clone();
        let session = project.update(cx, |p, cx| p.open_session(None, agent, window, cx));
        self.show_session(at, &session, window, cx);
        Some(session)
    }
}
