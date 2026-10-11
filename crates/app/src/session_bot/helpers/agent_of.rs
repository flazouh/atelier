use atelier_agents::registry::Agent;
use atelier_bots::Harness;

/// The agent that runs a bot's harness; `None` for a harness this build cannot start.
pub fn agent_of(harness: Harness) -> Option<Agent> {
    atelier_agents::registry::by_backend(backend_of(harness)?)
}

/// The name of the backend that runs a harness, as the registry names it.
pub(in super::super) fn backend_of(harness: Harness) -> Option<&'static str> {
    match harness {
        Harness::ClaudeCode => Some("claude-code"),
        Harness::Codex => Some("codex"),
        Harness::Cursor => Some("cursor"),
        Harness::Grok | Harness::Opencode | Harness::Custom => None,
    }
}
