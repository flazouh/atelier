//! The usage dashboard: what Claude Code and Codex spent on this machine, by account, day, model and session. The
//! data comes from [`atelier_agents::usage_history`] (the agents' own session logs); this module turns it, and the
//! limits the status bar already reads, into what [`atelier_ui::UsageDashboard`] draws. [`register`] adds the usage chips to the
//! status bar and the dashboard they open to the app; the page itself is [`UsagePage`].
mod consts;
mod helpers;
mod structs;
pub use helpers::register;
pub use structs::UsagePage;
#[cfg(test)]
pub use structs::UsageStore;
#[cfg(test)]
mod tests;
