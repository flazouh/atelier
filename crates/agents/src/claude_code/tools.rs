//! Claude Code's tool names, in one place: the only file that says which tool does what.

mod helpers;
mod types;

pub(super) use helpers::{edit_of, file, file_in_partial_input, kind, starts_subagent, streams_input, todo_tool};
pub(super) use types::TodoTool;
