//! What backends that run an agent as a child process share: starting it through the project, so a
//! remote project runs it on its host, and reading its stdout a line at a time. Not part of the
//! `Backend` trait: a backend with no process never touches this.

mod helpers;
mod types;

pub use types::CANCELLED;
pub use helpers::{exit_why, lines, output, run, start, stderr_tail, strip_ansi};

#[cfg(test)]
mod tests;
