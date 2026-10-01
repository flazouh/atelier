//! The automation that moves a task along, from the neutral events of sessions and pull requests. Each rule
//! is data: a name, a plain sentence for the settings, and an on/off switch in a [`RuleSet`]. The decision
//! ([`RuleSet::decide`]) is pure; [`handle`] reads the task, decides, and writes through a [`Tracker`].

mod helpers;
mod structs;
mod types;

pub use helpers::handle;
pub use structs::{Decision, Handled, RuleSet};
pub use types::{Rule, Signal};

#[cfg(test)]
mod tests;
