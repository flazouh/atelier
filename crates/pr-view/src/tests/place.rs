use atelier_forge::{Change, Side};

use crate::{
    diff::FileView,
    fixture::sample,
    git::{Blob, FileEntry},
    place::place,
};

fn view() -> FileView {
    let entry = FileEntry { path: "a.rs".into(), old_path: None, change: Change::Modified, additions: 0, deletions: 0, binary: false, old_blob: Some("a".into()), new_blob: Some("b".into()) };
    // Rows: 0 one | 1 two (old) | 2 TWO (new) | 3 three
    FileView::build(&entry, &Blob::Text("one\ntwo\nthree\n".into()), &Blob::Text("one\nTWO\nthree\n".into()))
}

fn thread(id: &str, path: &str, line: Option<u32>, side: Side) -> atelier_forge::Thread {
    let mut t = sample::thread(id, path, line.unwrap_or(0), vec![sample::comment("c", "Ada", "x", 1)]);
    t.line = line;
    t.side = side;
    t
}

#[test]
fn a_thread_hangs_under_the_row_of_its_line_on_its_side() {
    let threads = vec![thread("new", "a.rs", Some(2), Side::Right), thread("old", "a.rs", Some(2), Side::Left), thread("other file", "b.rs", Some(1), Side::Right)];
    let placement = place(&threads, &view());
    assert_eq!(placement.rows, vec![(1, 1), (2, 0)], "the old line 2 is row 1, the new line 2 is row 2");
    assert_eq!(placement.total(), 2, "a thread of another file is not this file's");
}

#[test]
fn threads_on_the_whole_file_and_outdated_ones_have_no_row() {
    let mut whole = thread("whole", "a.rs", None, Side::Right);
    whole.file_level = true;
    let mut gone = thread("gone", "a.rs", None, Side::Right);
    gone.outdated = true;
    let mut stale = thread("stale", "a.rs", Some(3), Side::Right);
    stale.outdated = true;
    let past_the_end = thread("past", "a.rs", Some(99), Side::Right);
    let placement = place(&[whole, gone, stale, past_the_end], &view());
    assert_eq!(placement.file_level, vec![0]);
    assert_eq!(placement.outdated, vec![1, 2, 3]);
    assert!(placement.rows.is_empty());
}

#[test]
fn several_threads_on_one_row_stay_in_order_and_a_renamed_file_keeps_its_old_threads() {
    let entry = FileEntry { path: "new.rs".into(), old_path: Some("old.rs".into()), change: Change::Renamed, additions: 0, deletions: 0, binary: false, old_blob: Some("a".into()), new_blob: Some("b".into()) };
    let view = FileView::build(&entry, &Blob::Text("x\ny\n".into()), &Blob::Text("x\ny\n".into()));
    let threads = vec![thread("t1", "new.rs", Some(2), Side::Right), thread("t2", "old.rs", Some(2), Side::Left), thread("t3", "new.rs", Some(1), Side::Right)];
    assert_eq!(place(&threads, &view).rows, vec![(0, 2), (1, 0), (1, 1)]);
}

#[test]
fn a_binary_file_lists_its_threads_above() {
    let entry = FileEntry { path: "logo.png".into(), old_path: None, change: Change::Modified, additions: 0, deletions: 0, binary: true, old_blob: None, new_blob: Some("b".into()) };
    let view = FileView::build(&entry, &Blob::Binary, &Blob::Binary);
    let mut whole = thread("whole", "logo.png", None, Side::Right);
    whole.file_level = true;
    let placement = place(&[whole, thread("line", "logo.png", Some(1), Side::Right)], &view);
    assert_eq!((placement.file_level.clone(), placement.outdated.clone()), (vec![0], vec![1]));
}
