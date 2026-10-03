//! Handing a session to another agent, or to the same agent on another provider, when it cannot resume the
//! session itself. The next agent gets a brief: where the work comes from, the conversation within a budget, and
//! where the whole transcript is kept. Both are made from a backend's neutral history, so any pair of agents works.

mod helpers;
mod structs;
mod types;

pub use helpers::{BUDGET, brief, transcript, turns};
pub use structs::{Origin, Turn};

#[cfg(test)]
mod tests;
