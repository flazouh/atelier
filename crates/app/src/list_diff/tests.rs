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
    let a = Item::Text { block: lathe_agents::session::BlockId(1), text: "Hel".into() };
    let b = Item::Text { block: lathe_agents::session::BlockId(1), text: "Hello".into() };
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
