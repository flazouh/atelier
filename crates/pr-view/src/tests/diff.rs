use atelier_forge::Change;

use crate::{
    diff::{Content, FileView, LineMap},
    git::{Blob, FileEntry},
};

fn entry(path: &str, change: Change, binary: bool) -> FileEntry {
    FileEntry { path: path.into(), old_path: None, change, additions: 0, deletions: 0, binary, old_blob: Some("aaaa".into()), new_blob: Some("bbbb".into()) }
}

fn text(s: &str) -> Blob {
    Blob::Text(s.into())
}

#[test]
fn both_sides_become_one_text_with_the_hunks_named() {
    let old = "one\ntwo\nthree\nfour\nfive\n";
    let new = "one\nTWO\nthree\nfour\nfive\nsix\n";
    let view = FileView::build(&entry("a.rs", Change::Modified, false), &text(old), &text(new));
    let shown = view.shown().unwrap();
    assert_eq!(shown.text(), "one\ntwo\nTWO\nthree\nfour\nfive\nsix\n", "the old row, then the new one");
    assert_eq!(shown.hunks().len(), 2);
    assert_eq!(shown.counts(), (2, 1));
    assert_eq!(view.version, "bbbb");
}

#[test]
fn an_added_file_is_all_new_rows_and_a_deleted_one_all_old() {
    let added = FileView::build(&entry("n.rs", Change::Added, false), &Blob::Missing, &text("a\nb\n"));
    assert_eq!(added.shown().unwrap().counts(), (2, 0));
    let deleted = FileView::build(&entry("d.rs", Change::Deleted, false), &text("a\nb\nc\n"), &Blob::Missing);
    assert_eq!(deleted.shown().unwrap().counts(), (0, 3));
}

#[test]
fn a_file_that_is_not_text_or_is_too_big_has_no_rows() {
    let binary = FileView::build(&entry("logo.png", Change::Modified, false), &Blob::Binary, &Blob::Binary);
    assert!(matches!(binary.content, Content::Binary));
    let flagged = FileView::build(&entry("blob", Change::Modified, true), &text("x"), &text("y"));
    assert!(matches!(flagged.content, Content::Binary), "git said it counts no lines");
    let big = FileView::build(&entry("big", Change::Modified, false), &Blob::TooLarge(9_000_000), &text("x"));
    assert!(matches!(big.content, Content::TooLarge(9_000_000)));
    assert!(binary.shown().is_none());
}

#[test]
fn a_thread_lands_on_the_row_of_its_line_on_either_side() {
    // Rows: 0 one | 1 two (old) | 2 TWO (new) | 3 three | 4 four (old) | 5 IV (new) | 6 five
    let view = FileView::build(&entry("a.rs", Change::Modified, false), &text("one\ntwo\nthree\nfour\nfive\n"), &text("one\nTWO\nthree\nIV\nfive\n"));
    let lines = &view.shown().unwrap().lines;
    assert_eq!(lines.head_row(1), Some(0));
    assert_eq!(lines.head_row(2), Some(2), "line 2 of the new file is the new row");
    assert_eq!(lines.head_row(4), Some(5));
    assert_eq!(lines.head_row(5), Some(6));
    assert_eq!(lines.head_row(6), None);
    assert_eq!(lines.head_row(0), None);
    assert_eq!(lines.base_row(2), Some(1), "line 2 of the old file is the old row");
    assert_eq!(lines.base_row(4), Some(4));
    assert_eq!(lines.base_row(5), Some(6));
    assert_eq!(lines.head_line(2), Some(2));
    assert_eq!(lines.head_line(1), None, "an old row has no line in the new file");
    assert_eq!(lines.base_line(1), Some(2));
    assert_eq!(lines.base_line(2), None, "a new row has no line in the old file");
}

#[test]
fn a_file_with_nothing_changed_maps_line_for_line() {
    let view = FileView::build(&entry("same.rs", Change::Modified, false), &text("a\nb\nc\n"), &text("a\nb\nc\n"));
    let lines: &LineMap = &view.shown().unwrap().lines;
    assert!(view.shown().unwrap().hunks().is_empty());
    assert_eq!((lines.head_row(3), lines.base_row(3)), (Some(2), Some(2)));
}

#[test]
fn the_row_map_for_the_language_server_agrees_with_the_line_map() {
    let view = FileView::build(&entry("a.rs", Change::Modified, false), &text("one\ntwo\nthree\n"), &text("one\nTWO\nthree\nfour\n"));
    let shown = view.shown().unwrap();
    for line in 1..=4u32 {
        let row = shown.lines.head_row(line).unwrap();
        assert_eq!(shown.rows.to_view(line as usize - 1), row, "line {line}");
    }
}

#[test]
fn a_big_file_diffs_quickly() {
    let old: String = (0..20_000).map(|i| format!("line {i}\n")).collect();
    let new: String = old.replacen("line 10000", "changed", 1).replacen("line 15000\n", "", 1);
    let start = std::time::Instant::now();
    let view = FileView::build(&entry("big.rs", Change::Modified, false), &text(&old), &text(&new));
    assert_eq!(view.shown().unwrap().hunks().len(), 2);
    assert!(start.elapsed() < std::time::Duration::from_secs(2), "{:?}", start.elapsed());
}
