use super::Reviewed;
use crate::FileReview;

fn file(path: &str, after: &str) -> FileReview {
    FileReview::from_texts(path, Some("old\n".into()), Some(after.into()), true)
}

#[test]
fn a_file_marked_in_a_turn_is_reviewed_in_that_turn_only() {
    let mut reviewed = Reviewed::new();
    let a = file("a.rs", "new\n");
    assert!(!reviewed.is_reviewed(0, &a));
    reviewed.mark(0, &a);
    assert!(reviewed.is_reviewed(0, &a));
    assert!(!reviewed.is_reviewed(1, &a), "another turn is another review");
    assert!(!reviewed.is_reviewed(0, &file("b.rs", "new\n")));
}

#[test]
fn a_mark_expires_when_the_file_changes_again() {
    let mut reviewed = Reviewed::new();
    reviewed.mark(0, &file("a.rs", "new\n"));
    assert!(!reviewed.is_reviewed(0, &file("a.rs", "newer\n")));
    assert!(reviewed.is_reviewed(0, &file("a.rs", "new\n")), "the same text is the same version");
}

#[test]
fn a_mark_can_be_taken_back() {
    let mut reviewed = Reviewed::new();
    let a = file("a.rs", "new\n");
    reviewed.mark(2, &a);
    reviewed.unmark(2, "a.rs");
    assert!(!reviewed.is_reviewed(2, &a));
}

#[test]
fn progress_counts_the_marked_files_of_a_turn() {
    let mut reviewed = Reviewed::new();
    let files = [file("a.rs", "1\n"), file("b.rs", "1\n"), file("c.rs", "1\n")];
    reviewed.mark(0, &files[0]);
    reviewed.mark(0, &files[2]);
    reviewed.mark(1, &files[1]);
    assert_eq!(reviewed.progress(0, &files), (2, 3));
    assert_eq!(reviewed.progress(1, &files), (1, 3));
    assert_eq!(reviewed.progress(0, &[]), (0, 0));
}
