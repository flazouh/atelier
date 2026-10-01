use crate::{
    fixture::sample,
    state::{Reviewed, is_seen},
};

#[test]
fn a_file_is_seen_for_the_version_it_was_marked_at() {
    let store = Reviewed::in_memory().unwrap();
    let pull = sample::reference(1);
    store.mark(&pull, "src/a.rs", "v1", 100).unwrap();
    let marks = store.marks(&pull).unwrap();
    assert!(is_seen(&marks, "src/a.rs", "v1"));
    assert!(!is_seen(&marks, "src/a.rs", "v2"), "a push that changed the file makes it unseen");
    assert!(!is_seen(&marks, "src/b.rs", "v1"));
    assert!(!is_seen(&marks, "src/a.rs", ""), "a file with no version is never seen");
}

#[test]
fn marking_again_takes_the_new_version_and_unmark_takes_it_back() {
    let store = Reviewed::in_memory().unwrap();
    let pull = sample::reference(1);
    store.mark(&pull, "a", "v1", 1).unwrap();
    store.mark(&pull, "a", "v2", 2).unwrap();
    assert_eq!(store.marks(&pull).unwrap().len(), 1);
    assert!(is_seen(&store.marks(&pull).unwrap(), "a", "v2"));
    store.unmark(&pull, "a").unwrap();
    assert!(store.marks(&pull).unwrap().is_empty());
    store.unmark(&pull, "a").unwrap();
}

#[test]
fn each_pull_request_keeps_its_own_marks_and_put_all_back_clears_one() {
    let store = Reviewed::in_memory().unwrap();
    let (one, two) = (sample::reference(1), sample::reference(2));
    store.mark(&one, "a", "v", 1).unwrap();
    store.mark(&one, "b", "v", 1).unwrap();
    store.mark(&two, "a", "v", 1).unwrap();
    assert_eq!(store.clear(&one).unwrap(), 2);
    assert!(store.marks(&one).unwrap().is_empty());
    assert_eq!(store.marks(&two).unwrap().len(), 1);
}

#[test]
fn the_marks_survive_a_restart_and_odd_paths_are_safe() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state").join("pr.sqlite");
    let pull = sample::reference(9);
    {
        let store = Reviewed::open(&path).unwrap();
        store.mark(&pull, "dir/it's \"odd\"; DROP TABLE reviewed;--.rs", "v", 1).unwrap();
    }
    let store = Reviewed::open(&path).unwrap();
    assert!(is_seen(&store.marks(&pull).unwrap(), "dir/it's \"odd\"; DROP TABLE reviewed;--.rs", "v"));
    store.mark(&pull, "x", "v", 2).unwrap();
    assert_eq!(store.marks(&pull).unwrap().len(), 2);
}

#[test]
fn a_database_from_a_newer_atelier_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pr.sqlite");
    drop(Reviewed::open(&path).unwrap());
    rusqlite::Connection::open(&path).unwrap().pragma_update(None, "user_version", 99).unwrap();
    assert!(Reviewed::open(&path).is_err());
}

#[test]
fn when_a_pull_request_was_last_opened_is_kept() {
    let store = Reviewed::in_memory().unwrap();
    let (one, two) = (sample::reference(1), sample::reference(2));
    assert!(store.opened_at().unwrap().is_empty());
    store.opened(&one, 100).unwrap();
    store.opened(&two, 200).unwrap();
    store.opened(&one, 300).unwrap();
    let opened = store.opened_at().unwrap();
    assert_eq!(opened.get(&(one.repo.slug(), 1)), Some(&300));
    assert_eq!(opened.get(&(two.repo.slug(), 2)), Some(&200));
}
