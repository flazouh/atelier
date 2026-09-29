use beui::InlineHunk;

use super::*;

/// Every holder of a session's rows sees a new map: the server's rows move with the review's hunks.
#[test]
fn the_rows_follow_a_new_map() {
    let rows = Rows::new(PathBuf::from("/p/a.rs"), RowMap::new(&[InlineHunk::new("h", 1..2, 2..3)]));
    let held = rows.clone();
    let at = |line| Position { line, character: 0 };
    assert_eq!(held.to_head(at(1)), None, "row 1 was removed");
    assert_eq!(held.to_head(at(2)), Some(at(1)));
    assert_eq!(held.doc("a\nold\nnew\nb").text, "a\nnew\nb");
    // The hunk was accepted: its removed row is gone from the buffer, and no row is removed now.
    *rows.map() = RowMap::default();
    assert_eq!(held.to_head(at(1)), Some(at(1)));
    assert_eq!(held.doc("a\nnew\nb").text, "a\nnew\nb");
}
