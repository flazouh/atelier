//! The agents lathe can start, as data, so the app offers them without naming one: each with its
//! backend, the name and mark its session rows wear, its look while it works, and the lab whose
//! mark its model picker shows.

use std::sync::Arc;

use beui::{AgentLook, BrandMark};

use crate::{claude, claude_code::ClaudeCode, coding_agents::CodingAgent, labs::Lab, own::OwnAgent, session::Backend};

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
    vec![
        Agent {
            backend: Arc::new(ClaudeCode::new()),
            name: CodingAgent::ClaudeCode.name(),
            mark: CodingAgent::ClaudeCode.mark(),
            look: claude::look(),
            lab: Lab::Anthropic,
        },
        // lathe's own agent. With no key set it still shows, and opening it says which to set. The
        // look is a stand-in until lathe has its own.
        Agent { backend: Arc::new(OwnAgent::from_env()), name: "lathe", mark: None, look: claude::look(), lab: Lab::Anthropic },
    ]
}

/// The lab that makes the model a picker names, by its id: `opus` and `claude-sonnet-4` are
/// Anthropic's, `gpt-5.2` and `o3` OpenAI's, `grok-4` xAI's.
pub fn model_lab(model_id: &str) -> Option<Lab> {
    let id = model_id.to_ascii_lowercase();
    let id = id.rsplit('/').next().unwrap_or(&id);
    if id.starts_with("claude") || ["opus", "sonnet", "haiku"].iter().any(|m| id.starts_with(m)) {
        Some(Lab::Anthropic)
    } else if id.starts_with("gpt") || id.starts_with("codex") || ["o1", "o3", "o4"].iter().any(|m| id == *m || id.starts_with(&format!("{m}-"))) {
        Some(Lab::OpenAi)
    } else if id.starts_with("grok") {
        Some(Lab::Xai)
    } else {
        None
    }
}

/// The mark beside a model in a picker; `None` for a model of no known lab, or a lab with no mark.
pub fn model_mark(model_id: &str) -> Option<BrandMark> {
    model_lab(model_id)?.mark()
}

/// The agent whose backend is named `backend`, as a session records it.
pub fn by_backend(backend: &str) -> Option<Agent> {
    agents().into_iter().find(|a| a.backend.name() == backend)
}

#[cfg(test)]
mod tests;
