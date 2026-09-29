use beui::changed_files::FileChange;

use super::{changed_files, hunks};
use crate::{Change, FileReview};

#[test]
fn the_changed_files_list_carries_counts_and_the_kind_of_change() {
    let files = [
        FileReview::from_texts("edit.rs", Some("1\n2\n".into()), Some("1\nX\nY\n".into()), true),
        FileReview::from_texts("new.rs", None, Some("a\nb\n".into()), true),
        FileReview::from_texts("gone.rs", Some("a\n".into()), None, true),
        FileReview::from_texts("to.rs", Some("same\n".into()), Some("same\n".into()), true).renamed("from.rs".into()),
        FileReview::binary("logo.png", Change::Modified),
    ];
    let list = changed_files(&files);
    let shape: Vec<_> = list.iter().map(|f| (f.path.to_string(), f.added, f.removed, f.change.clone())).collect();
    assert_eq!(
        shape,
        [
            ("edit.rs".into(), 2, 1, FileChange::Modified),
            ("new.rs".into(), 2, 0, FileChange::Added),
            ("gone.rs".into(), 0, 1, FileChange::Deleted),
            ("to.rs".into(), 0, 0, FileChange::Renamed { from: "from.rs".into() }),
            ("logo.png".into(), 0, 0, FileChange::Modified),
        ]
    );
}

#[test]
fn a_files_hunks_are_the_inline_reviews_and_a_binary_file_has_none() {
    let text = FileReview::from_texts("a.rs", Some("1\n2\n".into()), Some("1\nX\n".into()), true);
    let h = hunks(&text);
    assert_eq!(h.len(), 1);
    assert_eq!((h[0].removed.clone(), h[0].added.clone()), (1..2, 2..3));
    assert!(hunks(&FileReview::binary("a.png", Change::Added)).is_empty());
    assert!(hunks(&FileReview::unknown("a", None)).is_empty());
}
