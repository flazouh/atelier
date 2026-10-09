//! A session's panel: its header, what the agent said and did as a virtual list, the todos and the
//! subagents still running, and the composer. Every row comes from the session's `Conversation`, drawn
//! with the agent panel's own parts, so a live session and one read back from history look the same.
//!
//! The list lays out only the rows on screen; a question shows `ToolApproval` in its place in the
//! conversation, and its answer goes back through the session.

pub(crate) mod calls;
mod edit;
mod handoff_button;
mod helpers;
mod limit;
mod preview;
mod question;
mod rail;
mod sign_in;
mod summary;
mod tint;
mod tool_card;
mod types;

pub use helpers::{rows, session_view_with};
#[cfg(test)]
pub use types::READING_WIDTH;
#[cfg(test)]
pub use helpers::session_view;

#[cfg(test)]
use helpers::{gap_between, is_lookup, shows_stop};
#[cfg(test)]
use types::Block;

#[cfg(test)]
mod tests;
