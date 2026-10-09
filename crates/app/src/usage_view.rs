//! The usage dashboard: what Claude Code and Codex spent on this machine, by account, day, model and session. The
//! data comes from [`atelier_agents::usage_history`] (the agents' own session logs); this module turns it, and the
//! limits the status bar already reads, into what [`atelier_ui::UsageDashboard`] draws. It holds no window code: the
//! shell owns the state and the modal.
mod consts;
mod helpers;
mod structs;
pub use helpers::{build, today};
/// The longest range the dashboard shows, in days.
pub fn consts_longest() -> u32 {
    consts::LONGEST
}
pub use structs::UsageState;
#[cfg(test)]
mod tests;
