use std::sync::Arc;

use atelier_ui::BrandMark;

use crate::{
    acp::Acp, claude, claude_code::ClaudeCode, codex::Codex, coding_agents::CodingAgent, cursor, labs::Lab,
    own::OwnAgent,
};
use super::structs::Agent;

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
        // Cursor's agent over ACP. Its models come from several labs; each model's mark comes from its id.
        Agent {
            backend: Arc::new(Acp::new(cursor::agent())),
            name: CodingAgent::Cursor.name(),
            mark: CodingAgent::Cursor.mark(),
            look: cursor::look(),
            lab: Lab::Custom,
        },
        // Codex over ACP, through the adapter that runs its app server. Its models are OpenAI's.
        Agent {
            backend: Arc::new(Acp::new(Codex::agent())),
            name: CodingAgent::Codex.name(),
            mark: CodingAgent::Codex.mark(),
            look: Codex::look(),
            lab: Lab::OpenAi,
        },
        // atelier's own agent. With no key set it still shows, and opening it says which to set. The
        // look is a stand-in until atelier has its own.
        Agent { backend: Arc::new(OwnAgent::from_env()), name: "atelier", mark: None, look: claude::look(), lab: Lab::Anthropic },
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

/// The agent whose backend is named `backend`, as a session records it. A session recorded when atelier was
/// named lathe names atelier's own agent "lathe".
pub fn by_backend(backend: &str) -> Option<Agent> {
    let backend = if backend == "lathe" { "atelier" } else { backend };
    agents().into_iter().find(|a| a.backend.name() == backend)
}
