//! How much of each provider's allowance is used. A [`UsageSource`] reads one provider's windows (Claude's five
//! hours and seven days, Codex's own) as a [`Reading`]; the app shows them in the status bar. Each source reads
//! the sign-in the agent already has, through the project, so a remote project reports its own host's account.
//! The numbers come from the provider's own endpoint or app server; the reasons are in `docs/agents.md`.

mod consts;
mod impls;
mod structs;
mod traits;

pub use structs::{ClaudeUsage, CodexUsage, OpenRouterUsage, Reading, Window};
pub use traits::UsageSource;

#[cfg(test)]
mod tests;
