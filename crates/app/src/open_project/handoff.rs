//! Handing a session off: the targets this project offers, and the new session that carries another one on.

use atelier_ui::menu::Branch;
use gpui_kit::{AppContext, Context, Entity, Window};

use crate::agent_session::{AgentSession, handoff::Source};
use crate::handoff_targets::{Target, Targets};

use super::structs::OpenProject;
use super::types::ProjectEvent;

impl OpenProject {
    /// The tree of targets the handoff menus show.
    pub fn handoff_branches(&self) -> Vec<Branch> {
        self.handoff_targets.branches()
    }

    /// Reads the agents' accounts on this project's host and the keychain, off the UI thread, and gives the result to
    /// the sessions and the sidebar.
    pub fn read_handoff_targets(&mut self, cx: &mut Context<Self>) {
        let (agents, project, secrets) = (self.offered.clone(), self.project.clone(), crate::providers::secrets(cx));
        let reading = cx.background_spawn(async move { Targets::read(&agents, project.as_ref(), secrets.as_ref()) });
        self.reading_targets = cx.spawn(async move |this, cx| {
            let targets = reading.await;
            _ = this.update(cx, |this, cx| this.keep_handoff_targets(targets, cx));
        });
    }

    /// A test's own agents in place of the build's, and their targets read.
    #[cfg(test)]
    pub fn offer(&mut self, agents: Vec<atelier_agents::registry::Agent>, cx: &mut Context<Self>) {
        self.offered = agents;
        self.read_handoff_targets(cx);
    }

    fn keep_handoff_targets(&mut self, targets: Targets, cx: &mut Context<Self>) {
        if self.handoff_targets == targets {
            return;
        }
        self.handoff_targets = targets;
        let branches = self.handoff_branches();
        for session in &self.sessions {
            session.update(cx, |s, cx| {
                s.handoff_branches = branches.clone();
                cx.notify();
            });
        }
        cx.emit(ProjectEvent::Sessions);
    }

    /// Opens a new session on `target` that carries `source` on, and returns it; `None` when `target` names an agent
    /// this project does not offer.
    pub fn handoff(&mut self, source: Source, target: &Target, window: &mut Window, cx: &mut Context<Self>) -> Option<Entity<AgentSession>> {
        let agent = self.offered.iter().find(|agent| agent.backend.name() == target.backend)?.clone();
        let session = self.open_session(None, Some(agent), window, cx);
        session.update(cx, |s, cx| s.continue_from(source, target.provider.clone(), cx));
        Some(session)
    }
}
