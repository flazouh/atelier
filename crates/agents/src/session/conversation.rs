//! What the panel shows, built from events. The fold is pure: give it the events of a session in order
//! and it holds the items to draw, the todo list, the usage and whether the agent works. The UI keeps one
//! `Conversation` per session and calls `apply` for each event a frame drains, so drawing never depends on
//! which backend made the events.

mod structs;
mod types;

pub use structs::{Call, Conversation};
pub use types::{Answer, Item, SubagentStatus};
