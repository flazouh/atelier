use atelier_agents::session::{Call, Item, Subagent, SubagentStatus, ToolCall, ToolId, ToolKind, ToolStatus};

use super::super::tool_runs;

fn tool(status: ToolStatus) -> Item {
    let call = ToolCall { id: ToolId::new("t"), name: "Read".into(), kind: ToolKind::Read, input: serde_json::json!({}), file: None, parent: None, status };
    Item::Tool(Call { call, output: None, edit: None })
}

fn subagent(status: SubagentStatus) -> Item {
    let subagent = Subagent { id: ToolId::new("s"), task: "look".into(), kind: None, model: None };
    Item::Subagent { subagent, status, activity: None, summary: None, calls: Vec::new() }
}

fn said() -> Item {
    Item::User { text: "go on".into() }
}

#[test]
fn a_call_that_was_announced_or_runs_at_the_end_is_a_tool_that_runs() {
    assert!(tool_runs(&[said(), tool(ToolStatus::Running)]));
    assert!(tool_runs(&[said(), tool(ToolStatus::Pending)]));
    assert!(tool_runs(&[said(), tool(ToolStatus::Running), tool(ToolStatus::Done)]), "calls made together: one still runs");
    assert!(tool_runs(&[subagent(SubagentStatus::Running)]));
}

#[test]
fn no_call_or_only_calls_that_ended_is_no_tool_that_runs() {
    assert!(!tool_runs(&[]));
    assert!(!tool_runs(&[said()]));
    assert!(!tool_runs(&[said(), tool(ToolStatus::Done), tool(ToolStatus::Failed)]));
    assert!(!tool_runs(&[subagent(SubagentStatus::Done)]));
}

#[test]
fn a_call_left_open_by_a_stopped_turn_does_not_count_once_anything_follows_it() {
    assert!(!tool_runs(&[tool(ToolStatus::Running), said()]));
}
