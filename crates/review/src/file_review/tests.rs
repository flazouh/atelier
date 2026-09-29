use super::{Change, Content, FileReview};

#[test]
fn a_file_with_text_on_both_sides_is_modified() {
    let file = FileReview::from_texts("a.rs", Some("1\n".into()), Some("2\n".into()), true);
    assert_eq!(file.change, Change::Modified);
    assert_eq!(file.counts(), (1, 1));
}

#[test]
fn a_missing_side_makes_a_file_added_or_deleted() {
    assert_eq!(FileReview::from_texts("a", None, Some("x\n".into()), true).change, Change::Added);
    assert_eq!(FileReview::from_texts("a", Some("x\n".into()), None, true).change, Change::Deleted);
}

#[test]
fn the_version_changes_with_the_text_now_and_only_with_it() {
    let a = FileReview::from_texts("a", Some("1\n".into()), Some("2\n".into()), true);
    let same_after = FileReview::from_texts("a", Some("other before\n".into()), Some("2\n".into()), true);
    let other_after = FileReview::from_texts("a", Some("1\n".into()), Some("3\n".into()), true);
    assert_eq!(a.version(), same_after.version());
    assert_ne!(a.version(), other_after.version());
}

#[test]
fn a_file_with_no_text_counts_nothing() {
    assert_eq!(FileReview::binary("a.png", Change::Added).counts(), (0, 0));
    let unknown = FileReview::unknown("a", Some("x\n".into()));
    assert_eq!((unknown.counts(), &unknown.content, unknown.exact), ((0, 0), &Content::Unknown, false));
}
