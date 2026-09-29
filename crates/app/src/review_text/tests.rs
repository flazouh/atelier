use super::*;

#[test]
fn a_splice_replaces_only_what_differs() {
    let (old, new) = ("a\nb\nc\n", "a\nB2\nc\n");
    let (range, with) = splice(old, new);
    assert_eq!((range.clone(), with), (2..3, "B2"));
    let mut text = old.to_string();
    text.replace_range(range, with);
    assert_eq!(text, new);
    assert_eq!(splice("same", "same"), (4..4, ""));
    // A repeated letter at the seam is not counted twice.
    let (range, with) = splice("aa", "aaa");
    assert_eq!(range.len() + 3 - with.len(), 2);
}

#[test]
fn a_splice_keeps_to_char_boundaries() {
    let (old, new) = ("é", "è");
    let (range, with) = splice(old, new);
    let mut text = old.to_string();
    text.replace_range(range, with);
    assert_eq!(text, new);
}

#[test]
fn the_caret_keeps_its_text() {
    let edit = 10..14;
    assert_eq!(moved_caret(3, &edit, 1), 3, "before the edit");
    assert_eq!(moved_caret(20, &edit, 1), 17, "after it, moved by the change in length");
    assert_eq!(moved_caret(12, &edit, 1), 11, "inside it, to the end of the new text");
}

#[test]
fn a_comment_line_finds_its_row_on_either_side() {
    // "a\nb\nc\n" to "a\nB\nc\n": the merged rows are a, b (removed), B (added), c.
    let merged = Merged::diff("a\nb\nc\n", "a\nB\nc\n");
    assert_eq!(merged.text(), "a\nb\nB\nc\n");
    assert_eq!(row_of(&merged, Side::Current, 2), Some(2), "B is the current line 2");
    assert_eq!(row_of(&merged, Side::Current, 3), Some(3));
    assert_eq!(row_of(&merged, Side::Removed, 2), Some(1), "b was line 2 before the turn");
    assert_eq!(row_of(&merged, Side::Current, 0), None);
    assert_eq!(row_of(&merged, Side::Current, 9), None);
}
