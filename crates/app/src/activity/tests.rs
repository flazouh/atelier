use std::time::Duration;

use atelier_agents::session::{BlockId, Call, Item, SubagentStatus, ToolCall, ToolId, ToolKind, ToolStatus};

use super::*;

fn tool(name: &str) -> Item {
    kind_of(name, ToolKind::Other)
}

fn kind_of(name: &str, kind: ToolKind) -> Item {
    Item::Tool(Call {
        call: ToolCall { id: ToolId::new(name), name: name.into(), kind, input: serde_json::json!({}), file: None, parent: None, status: ToolStatus::Done },
        output: None,
    })
}

fn think(seconds: u64) -> Item {
    Item::Thinking { block: BlockId(1), text: "hm".into(), took: Some(Duration::from_secs(seconds)) }
}

fn subagent(calls: Vec<Call>) -> Item {
    Item::Subagent {
        subagent: atelier_agents::session::Subagent { id: ToolId::new("s"), kind: None, task: "t".into(), model: None },
        status: SubagentStatus::Done,
        activity: None,
        summary: None,
        calls,
    }
}

#[test]
fn thinking_time_and_the_calls_of_each_kind_are_counted() {
    let items = [
        think(4),
        kind_of("e", ToolKind::Edit),
        kind_of("g", ToolKind::Search),
        think(8),
        kind_of("g2", ToolKind::Search),
        kind_of("r1", ToolKind::Read),
        kind_of("r2", ToolKind::Read),
        kind_of("r3", ToolKind::Read),
        kind_of("r4", ToolKind::Read),
    ];
    assert_eq!(summary(&items, 0, items.len(), &|_| true), "Thought for 12s · Edited 1 · Searched 2 · Read 4");
}

#[test]
fn every_kind_has_its_word_in_a_set_order() {
    let items = [
        kind_of("a", ToolKind::Fetch),
        kind_of("b", ToolKind::Other),
        kind_of("c", ToolKind::Shell),
        kind_of("d", ToolKind::Write),
        kind_of("e", ToolKind::Edit),
    ];
    assert_eq!(summary(&items, 0, items.len(), &|_| true), "Edited 1 · Wrote 1 · Ran 1 · Fetched 1 · Used 1");
}

#[test]
fn a_subagent_counts_itself_and_the_calls_it_made() {
    assert_eq!(summary(&[subagent(vec![])], 0, 1, &|_| true), "1 subagent");
    let inner = |kind| match kind_of("x", kind) {
        Item::Tool(call) => call,
        _ => unreachable!(),
    };
    let items = [subagent(vec![inner(ToolKind::Read), inner(ToolKind::Read)]), kind_of("r", ToolKind::Read)];
    assert_eq!(summary(&items, 0, 2, &|_| true), "Read 3 · 1 subagent");
}

#[test]
fn an_item_that_draws_nothing_is_not_counted() {
    let items = [tool("a"), tool("b"), tool("c")];
    assert_eq!(summary(&items, 0, 3, &|ix| ix != 1), "Used 2");
}

#[test]
fn a_run_with_nothing_to_count_is_said_as_worked() {
    assert_eq!(summary(&[], 0, 0, &|_| true), "Worked");
}
