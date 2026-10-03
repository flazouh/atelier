use std::collections::{HashMap, HashSet};

use crate::session::{ContextFill, Limit, RequestId, Todo, ToolId};

use super::{super::enums::Open, Asked};

/// Reads `claude`'s stream-json lines. See [`LineMapper`](super::super::LineMapper).
#[derive(Default)]
pub struct ClaudeLineMapper {
    pub(in super::super) next_block: u64,
    /// Messages that streamed: their finished copy repeats what the deltas already said.
    pub(in super::super) streamed: HashSet<String>,
    pub(in super::super) open: HashMap<u32, Open>,
    /// Calls announced and not finished.
    pub(in super::super) running: HashSet<ToolId>,
    /// Calls whose result atelier swallows: todo edits and subagent starts.
    pub(in super::super) hidden: HashSet<ToolId>,
    pub(in super::super) subagents: HashSet<ToolId>,
    /// Shell commands that run on after their call returned (`run_in_background`), until `claude` says
    /// they ended. Each is a shell call that stays running, not a subagent.
    pub(in super::super) background: HashSet<ToolId>,
    /// The last turn ended aborted.
    pub(in super::super) aborted: bool,
    pub(in super::super) todos: Vec<Todo>,
    /// `TaskCreate` calls waiting for the id the result gives the task.
    pub(in super::super) creating: HashMap<ToolId, String>,
    pub(in super::super) asked: HashMap<RequestId, Asked>,
    pub(in super::super) turn_open: bool,
    /// The ids of atelier's messages `claude` has not taken into a turn yet.
    pub(in super::super) waiting: Vec<String>,
    pub(in super::super) ended: bool,
    pub(in super::super) context: ContextFill,
    pub(in super::super) limit: Option<Limit>,
    /// The model of the latest main-thread reply, whose window the context fills.
    pub(in super::super) model: Option<String>,
}
