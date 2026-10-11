use atelier_agents::session::{Item, SubagentStatus, ToolStatus};

/// Whether the agent runs a tool now: one of the calls at the end of the conversation was announced or runs, or a
/// subagent there runs. A call left open by a turn that was stopped does not count once anything follows it.
pub fn tool_runs(items: &[Item]) -> bool {
    items
        .iter()
        .rev()
        .take_while(|item| matches!(item, Item::Tool(_) | Item::Subagent { .. }))
        .any(|item| match item {
            Item::Tool(call) => matches!(call.call.status, ToolStatus::Pending | ToolStatus::Running),
            Item::Subagent { status, .. } => *status == SubagentStatus::Running,
            _ => false,
        })
}
