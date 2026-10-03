//! Claude Code's own record of past sessions: one JSON-lines file per session in
//! `~/.claude/projects/<the project's folder, with every other character as "-">/`. The files live on the
//! project's host, so they are read through a process the project spawns, never straight from disk.

mod helpers;
mod types;

pub use helpers::history;
pub(super) use helpers::{holder, list, read_history};
#[cfg(test)]
pub(super) use helpers::{holder_account, holder_script, list_script, parse_listing, read_script, slug};
