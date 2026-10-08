use super::SessionUsage;
use crate::usage_history::types::Provider;

/// One account: a Claude config folder or a Codex home, with the sessions found in it.
#[derive(Debug, Clone, PartialEq)]
pub struct AccountUsage {
    pub provider: Provider,
    /// The folder name without its leading dot: "claude", "claude-work", "codex".
    pub label: String,
    /// Newest first.
    pub sessions: Vec<SessionUsage>,
}
