use super::*;

#[test]
fn only_the_rows_that_changed_are_replaced_from_the_end() {
    let f = |n: usize| (1u8, n, 0usize);
    let before = [f(1), f(2), f(3), f(4)];
    assert!(changes(&before, &before).is_empty(), "nothing changed");
    // The last row grew, and two rows were added.
    let after = [f(1), f(2), f(3), f(40), f(5), f(6)];
    assert_eq!(changes(&before, &after), [(4..4, 2), (3..4, 1)]);
    // A row in the middle changed: a tool finished while text streamed at the end.
    let after = [f(1), f(20), f(3), f(40)];
    assert_eq!(changes(&before, &after), [(3..4, 1), (1..2, 1)]);
    // Neighbours that changed go as one range.
    let after = [f(10), f(20), f(3), f(4)];
    assert_eq!(changes(&before, &after), [(0..2, 2)]);
}

#[test]
fn a_streaming_text_changes_its_fingerprint() {
    let a = Item::Text { block: atelier_agents::session::BlockId(1), text: "Hel".into() };
    let b = Item::Text { block: atelier_agents::session::BlockId(1), text: "Hello".into() };
    assert_ne!(fingerprint(&a), fingerprint(&b));
}

#[test]
fn a_turns_card_follows_the_item_its_turn_ended_at() {
    use Row::*;
    assert_eq!(rows(0, &[]), []);
    assert_eq!(rows(3, &[(2, 0), (3, 1)]), [Item(0), Item(1), Changes { turn: 0 }, Item(2), Changes { turn: 1 }]);
    // Two turns that ended at the same item keep their order.
    assert_eq!(rows(1, &[(1, 0), (1, 1)]), [Item(0), Changes { turn: 0 }, Changes { turn: 1 }]);
}

/// A resumed session's history can hold fewer items than the live turn did (it keeps no questions), so
/// a card kept for a later item goes at the end rather than nowhere.
#[test]
fn a_card_past_the_last_item_goes_at_the_end() {
    use Row::*;
    assert_eq!(rows(2, &[(5, 0)]), [Item(0), Item(1), Changes { turn: 0 }]);
}

fn tool(name: &str) -> Item {
    use atelier_agents::session::{Call, ToolCall, ToolId, ToolKind, ToolStatus};
    Item::Tool(Call {
        call: ToolCall { id: ToolId::new(name), name: name.into(), kind: ToolKind::Other, input: serde_json::json!({}), file: None, parent: None, status: ToolStatus::Done },
        output: None,
        edit: None,
    })
}

fn think() -> Item {
    Item::Thinking { block: atelier_agents::session::BlockId(1), text: "hm".into(), took: None }
}

fn said() -> Item {
    Item::Text { block: atelier_agents::session::BlockId(2), text: "ok".into() }
}

/// Two or more activity items in a row are one group; one alone stays a row; what is said ends a group.
#[test]
fn a_run_of_activity_is_one_row_and_a_lone_item_is_not() {
    use Row::{Activity, Item as At};
    let items = [Item::User { text: "go".into() }, think(), tool("a"), tool("b"), said(), tool("c"), said()];
    assert_eq!(grouped(&items, &|_| true, &[]), [At(0), Activity { from: 1, to: 4 }, At(4), At(5), At(6)]);
}

/// A turn's card of changed files ends a group, since it sits between two items.
#[test]
fn a_changes_card_ends_a_group() {
    use Row::{Activity, Changes};
    let items = [think(), tool("a"), tool("b"), tool("c")];
    assert_eq!(grouped(&items, &|_| true, &[(2, 0)]), [Activity { from: 0, to: 2 }, Changes { turn: 0 }, Activity { from: 2, to: 4 }]);
}

/// An item that draws no row (a call waiting on its approval) does not count toward the two.
#[test]
fn items_that_draw_nothing_do_not_make_a_group() {
    use Row::Item as At;
    let items = [tool("a"), tool("b")];
    assert_eq!(grouped(&items, &|ix| ix == 0, &[]), [At(0), At(1)]);
}

/// A group's fingerprint moves when an item in it changes, when it goes live, and when it opens.
#[test]
fn a_groups_fingerprint_follows_its_items_and_its_state() {
    let items = [think(), tool("a")];
    let base = activity_fingerprint(&items, 0, 2, false, false);
    assert_ne!(base, activity_fingerprint(&items, 0, 2, true, false), "live");
    assert_ne!(base, activity_fingerprint(&items, 0, 2, false, true), "open");
    let grown = [think(), tool("a"), tool("b")];
    assert_ne!(base, activity_fingerprint(&grown, 0, 3, false, false), "a third item");
    assert_eq!(base, activity_fingerprint(&items, 0, 2, false, false), "same state, same print");
}

#[test]
fn a_row_arrives_once_even_when_it_joins_a_group() {
    let before = [Row::Item(0), Row::Item(1)];
    let after = [Row::Item(0), Row::Activity { from: 1, to: 3 }, Row::Changes { turn: 0 }, Row::Item(3)];
    assert_eq!(arrivals(&before, &after), vec![Arrival::Card(0), Arrival::Item(3)]);
    assert!(arrivals(&after, &after).is_empty(), "a group that grows is the same row");
}

/// An edit's text streams into its call, so the row measures again as the text grows.
#[test]
fn a_tool_calls_fingerprint_follows_its_input_as_it_streams() {
    use atelier_agents::session::{Call, ToolCall, ToolId, ToolKind, ToolStatus};
    let edit = |new: &str| {
        Item::Tool(Call {
            call: ToolCall {
                id: ToolId::new("t"),
                name: "Edit".into(),
                kind: ToolKind::Edit,
                input: serde_json::json!({"file_path": "/w/a.rs", "old_string": "a", "new_string": new}),
                file: None,
                parent: None,
                status: ToolStatus::Running,
            },
            output: None,
            edit: None,
        })
    };
    assert_ne!(fingerprint(&edit("b")), fingerprint(&edit("b\nc")));
    assert_eq!(fingerprint(&edit("b")), fingerprint(&edit("b")));
}
