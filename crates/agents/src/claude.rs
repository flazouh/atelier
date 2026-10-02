//! Claude's look: its spark, the Claude Code CLI's clay colours, and Claude's words for each phase.

mod helpers;
pub mod spark;
mod types;

pub use spark::SparkState;

pub use helpers::{labels, look, mark, spark};
pub use types::{CLAY, GLIMMER_CLAY, MESSAGE_CLAY};

#[cfg(test)]
use helpers::color;

#[cfg(test)]
mod tests;
