use std::time::Duration;

use super::super::event::{BlockId, ChoiceKind, PermissionRequest, Subagent};
use super::structs::Call;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubagentStatus {
    Running,
    Done,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The question waits for the user.
    Asking,
    Answered(ChoiceKind),
    /// The agent withdrew it, or the turn ended first.
    Withdrawn,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    User { text: String },
    Text { block: BlockId, text: String },
    Thinking { block: BlockId, text: String, took: Option<Duration> },
    Tool(Call),
    Subagent {
        subagent: Subagent,
        status: SubagentStatus,
        /// What it does now.
        activity: Option<String>,
        summary: Option<String>,
        /// Its own tool calls, in order.
        calls: Vec<Call>,
    },
    Permission { request: PermissionRequest, answer: Answer },
    /// A problem the user should see: a warning, or a turn that failed.
    Notice(String),
}

/// Where a tool call lives: at the top level, or inside a subagent.
#[derive(Clone, Copy, Debug)]
pub(super) enum Slot {
    Top(usize),
    Sub(usize, usize),
}
