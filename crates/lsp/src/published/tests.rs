use super::*;

fn error(message: &str) -> Diagnostic {
    Diagnostic { message: message.into(), ..Default::default() }
}

fn messages(found: Option<&[Diagnostic]>) -> Option<Vec<&str>> {
    found.map(|set| set.iter().map(|d| d.message.as_str()).collect())
}

#[test]
fn a_set_for_our_version_or_a_later_one_is_current() {
    let mut published = Published::default();
    published.store("a.rs".into(), Some(2), vec![error("two")]);
    assert_eq!(messages(published.current(Path::new("a.rs"), 2)), Some(vec!["two"]));
    assert_eq!(messages(published.current(Path::new("a.rs"), 1)), Some(vec!["two"]), "later than asked is fine");
}

#[test]
fn a_set_the_server_published_before_our_edit_is_stale() {
    let mut published = Published::default();
    published.store("a.rs".into(), Some(1), vec![error("old")]);
    assert_eq!(published.current(Path::new("a.rs"), 2), None);
}

#[test]
fn a_set_for_another_file_is_not_ours() {
    let mut published = Published::default();
    published.store("b.rs".into(), Some(2), vec![error("b")]);
    assert_eq!(published.current(Path::new("a.rs"), 2), None);
}

#[test]
fn an_unversioned_set_counts_only_until_the_next_change() {
    let mut published = Published::default();
    published.store("a.py".into(), None, vec![error("before the edit")]);
    assert!(published.current(Path::new("a.py"), 1).is_some(), "taken at its word");
    published.synced(Path::new("a.py"));
    assert_eq!(published.current(Path::new("a.py"), 2), None, "a set from before the edit is gone");
    published.store("a.py".into(), None, vec![error("after")]);
    assert_eq!(messages(published.current(Path::new("a.py"), 2)), Some(vec!["after"]));
}

#[test]
fn a_late_older_set_does_not_replace_a_newer_one() {
    let mut published = Published::default();
    published.store("a.rs".into(), Some(3), vec![error("three")]);
    published.store("a.rs".into(), Some(2), vec![error("two")]);
    assert_eq!(messages(published.current(Path::new("a.rs"), 3)), Some(vec!["three"]));
}
