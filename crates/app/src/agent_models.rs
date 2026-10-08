//! How the reader arranged each agent's models: the one new sessions start on and the order they are listed in, and the list an agent
//! reports of its own models (Claude Code asks Anthropic, so a release shows by its name with no change here). The settings keep it; this is
//! the copy every session and the Settings page read at once.
mod helpers;
mod structs;
pub use helpers::{all, choose_default, default_model, offered, refresh, reorder, reorder_visible, set_hidden, start_model};
pub use structs::ModelPrefs;
#[cfg(test)]
mod tests;
