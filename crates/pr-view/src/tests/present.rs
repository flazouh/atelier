use lathe_forge::{Change, PullState, Verdict};

use crate::{
    fixture::sample,
    git::{Commit, FileEntry},
    present::*,
};
use beui::{changed_files::FileChange, verdict::Decision};

#[test]
fn files_carry_their_counts_and_what_happened_to_them() {
    let entry = |path: &str, old: Option<&str>, change| FileEntry { path: path.into(), old_path: old.map(str::to_string), change, additions: 4, deletions: 2, binary: false, old_blob: None, new_blob: None };
    let files = changed_files(&[entry("a.rs", None, Change::Modified), entry("b.rs", None, Change::Added), entry("c.rs", None, Change::Deleted), entry("d.rs", Some("old.rs"), Change::Renamed), entry("e.rs", None, Change::Renamed)]);
    assert_eq!(files.iter().map(|f| (f.added, f.removed)).collect::<Vec<_>>(), vec![(4, 2); 5]);
    assert_eq!(files[1].change, FileChange::Added);
    assert_eq!(files[2].change, FileChange::Deleted);
    assert_eq!(files[3].change, FileChange::Renamed { from: "old.rs".into() });
    assert_eq!(files[4].change, FileChange::Modified, "a rename with no old name is shown as an edit");
}

#[test]
fn commits_say_how_long_ago() {
    let commits = commit_data(&[Commit { sha: "abc1234".into(), title: "Fix".into(), author: "Rui".into(), at: sample::NOW - 7300 }], sample::NOW);
    assert_eq!((commits[0].age.as_ref(), commits[0].title.as_ref(), commits[0].at), ("2h ago", "Fix", sample::NOW - 7300));
}

#[test]
fn a_thread_shows_its_people_once_each_its_first_words_and_whether_it_is_resolved() {
    let mut t = sample::thread("T", "a.rs", 3, vec![sample::comment("1", "Ada", "\n\n  Why is this here?\nMore.", sample::NOW - 60), sample::comment("2", "Rui", "Old code.", sample::NOW - 30), sample::comment("3", "Ada", "Thanks", sample::NOW)]);
    t.resolved = true;
    let s = thread_summary(&t, sample::NOW);
    assert_eq!(s.people.iter().map(|p| p.as_ref()).collect::<Vec<_>>(), ["Ada", "Rui"]);
    assert_eq!(s.first.as_ref(), "Why is this here?");
    assert!(s.resolved && s.comments.len() == 3);
    assert_eq!(s.comments[0].time.as_ref(), "1m ago");
}

#[test]
fn first_words_are_cut_at_ninety_characters() {
    assert_eq!(first_words("").as_ref(), "");
    let long = "x".repeat(200);
    let cut = first_words(&long);
    assert_eq!(cut.chars().count(), 91);
    assert!(cut.ends_with('…'));
    assert_eq!(first_words(&"y".repeat(90)).chars().count(), 90);
}

#[test]
fn a_remark_carries_its_author_and_first_words() {
    let r = remark_summary(&sample::comment("r", "canary", "Built at 5b2c1a9.\nDownload it.", sample::NOW - 3 * 3600), sample::NOW);
    assert_eq!((r.author.as_ref(), r.first.as_ref(), r.comment.time.as_ref()), ("canary", "Built at 5b2c1a9.", "3h ago"));
}

#[test]
fn what_the_reader_said_comes_from_the_opinions() {
    let mut pull = sample::pull(&sample::reference(1), "abcd");
    assert_eq!(stated_verdict(&pull, "alex"), None);
    pull.opinions = vec![lathe_forge::Opinion { reviewer: "ada".into(), verdict: Verdict::Approve }, lathe_forge::Opinion { reviewer: "alex".into(), verdict: Verdict::RequestChanges }];
    assert_eq!(stated_verdict(&pull, "alex"), Some((Decision::ChangesRequested, true)));
    assert_eq!(stated_verdict(&pull, "nobody"), None);
    pull.state = PullState::Merged;
}
