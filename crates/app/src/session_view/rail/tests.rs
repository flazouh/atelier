use atelier_agents::session::{BlockId, Item};

use super::*;

fn said(text: &str) -> Item {
    Item::User { text: text.into() }
}
fn wrote(text: &str) -> Item {
    Item::Text { block: BlockId(1), text: text.into() }
}

/// One entry for each message the reader sent, with the answer's start, at the row that shows it.
#[test]
fn each_message_of_the_reader_is_an_entry_with_the_start_of_its_answer() {
    let items = [said("fix the bug"), wrote("I found it in the parser."), said("and the tests"), Item::Notice("x".into())];
    let shown: Vec<Row> = (0..items.len()).map(Row::Item).collect();
    let got = entries(&items, &shown);
    assert_eq!(got.len(), 2);
    assert_eq!((got[0].0.label.as_ref(), got[0].0.description.as_deref(), got[0].1), ("fix the bug", Some("I found it in the parser."), 0));
    assert_eq!((got[1].0.label.as_ref(), got[1].0.description.as_deref(), got[1].1), ("and the tests", None, 2), "no answer yet: no description");
}

/// A group of rows can stand where items were: an entry's row is where its message is, not its item's number.
#[test]
fn an_entrys_row_is_where_the_list_shows_its_message() {
    let items = [said("one"), wrote("a"), said("two")];
    let shown = [Row::Item(0), Row::Item(1), Row::Changes { turn: 0 }, Row::Item(2)];
    assert_eq!(entries(&items, &shown).iter().map(|e| e.1).collect::<Vec<_>>(), [0, 3]);
}

#[test]
fn the_active_entry_follows_the_top_row_and_the_end() {
    let rows = [0, 4, 9];
    assert_eq!(active(&rows, 5, false), 1, "the last message at or above the top row");
    assert_eq!(active(&rows, 0, false), 0);
    assert_eq!(active(&rows, 20, false), 2);
    assert_eq!(active(&rows, 2, true), 2, "following the output: the last");
    assert_eq!(active(&[], 3, false), 0);
}
