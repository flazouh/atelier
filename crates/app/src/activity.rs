//! The words of an activity group's summary row: what the agent did in a run of thinking, tool calls and
//! subagents, in one line ("Thought for 12s · 3 tool calls"). Pure.

mod helpers;
mod types;

pub use helpers::summary;
pub use types::LIVE_HEIGHT;

#[cfg(test)]
mod tests;
