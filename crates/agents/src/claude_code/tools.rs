//! Claude Code's tool names, in one place: the only file that says which tool does what.

mod helpers;
mod types;

pub(super) use helpers::{file, file_in_partial_input, kind, starts_subagent, todo_tool};
pub(super) use types::TodoTool;
