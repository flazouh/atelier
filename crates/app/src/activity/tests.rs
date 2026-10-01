use std::time::Duration;

use atelier_agents::session::{BlockId, Call, Item, SubagentStatus, ToolCall, ToolId, ToolKind, ToolStatus};

use super::*;

fn tool(name: &str) -> Item {
    Item::Tool(Call {
        call: ToolCall { id: ToolId::new(name), name: name.into(), kind: ToolKind::Other, input: serde_json::json!({}), file: None, parent: None, status: ToolStatus::Done },
        output: None,
    })
}

fn think(seconds: u64) -> Item {
    Item::Thinking { block: BlockId(1), text: "hm".into(), took: Some(Duration::from_secs(seconds)) }
}

#[test]
fn thinking_time_and_tool_calls_are_counted() {
    let items = [think(4), tool("a"), think(8), tool("b"), tool("c")];
    assert_eq!(summary(&items, 0, 5, &|_| true), "Thought for 12s · 3 tool calls");
}

#[test]
fn one_tool_is_singular_and_a_subagent_counts_its_calls() {
    assert_eq!(summary(&[tool("a"), tool("b")], 0, 1, &|_| true), "1 tool call");
    let sub = Item::Subagent {
        subagent: atelier_agents::session::Subagent { id: ToolId::new("s"), kind: None, task: "t".into(), model: None },
        status: SubagentStatus::Done,
        activity: None,
        summary: None,
        calls: vec![],
    };
    assert_eq!(summary(&[sub], 0, 1, &|_| true), "1 subagent");
}

#[test]
fn an_item_that_draws_nothing_is_not_counted() {
    let items = [tool("a"), tool("b"), tool("c")];
    assert_eq!(summary(&items, 0, 3, &|ix| ix != 1), "2 tool calls");
}
