//! The agents lathe can start, as data, so the app offers them without naming one: each with its
//! backend, the name and mark its session rows wear, its look while it works, and the lab whose
//! mark its model picker shows.

use std::sync::Arc;

use beui::{AgentLook, BrandMark};

use crate::{claude, claude_code::ClaudeCode, coding_agents::CodingAgent, labs::Lab, session::Backend};

#[derive(Clone)]
pub struct Agent {
    pub backend: Arc<dyn Backend>,
    /// What a person calls it, for a session row or a menu.
    pub name: &'static str,
    /// Its mark, or `None` for a monogram.
    pub mark: Option<BrandMark>,
    pub look: AgentLook,
    /// The lab whose models it runs, for the marks beside them.
    pub lab: Lab,
}

/// Every agent this build can start, the default first.
pub fn agents() -> Vec<Agent> {
    vec![Agent {
        backend: Arc::new(ClaudeCode::new()),
        name: CodingAgent::ClaudeCode.name(),
        mark: CodingAgent::ClaudeCode.mark(),
        look: claude::look(),
        lab: Lab::Anthropic,
    }]
}

/// The agent whose backend is named `backend`, as a session records it.
pub fn by_backend(backend: &str) -> Option<Agent> {
    agents().into_iter().find(|a| a.backend.name() == backend)
}

#[cfg(test)]
mod tests;
