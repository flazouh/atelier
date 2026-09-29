use std::path::Path;

use super::repo::{Scenario, put};
use crate::base::{BaseChoice, opening_choice, resolve};

fn setup() -> (Scenario, String, Vec<String>) {
    let s = Scenario::new(&[("src/a.rs", "a\n")]);
    let base = s.main_tip();
    let head = s.pull(3, "main", &[&|r: &Path| put(r, "src/a.rs", "1\n"), &|r: &Path| put(r, "src/b.rs", "2\n")]);
    let (_, pull) = s.pull_data(3, &head, &base);
    let prgit = s.prgit();
    let prepared = prgit.prepare(&pull).unwrap();
    let commits = prgit.commits(&prepared, &base).unwrap().into_iter().map(|c| c.sha).collect();
    (s, head, commits)
}

#[test]
fn it_opens_since_the_last_review_only_when_there_is_something_new() {
    let head = "d405137bce56a332bfa8e2d7802ac69007d11c57";
    assert_eq!(opening_choice(None, head), BaseChoice::Whole);
    assert_eq!(opening_choice(Some("abcdef1234"), head), BaseChoice::LastReview);
    assert_eq!(opening_choice(Some(head), head), BaseChoice::Whole, "reviewed the head: nothing new, show all");
    assert_eq!(opening_choice(Some("not a sha"), head), BaseChoice::Whole);
}

#[test]
fn the_whole_pull_request_starts_at_the_merge_base_and_since_last_review_at_the_review_point() {
    let (s, head, commits) = setup();
    let prgit = s.prgit();
    let (_, pull) = s.pull_data(3, &head, &s.main_tip());
    let prepared = prgit.prepare(&pull).unwrap();
    let whole = resolve(&prgit, &prepared, &BaseChoice::Whole, None);
    assert_eq!((whole.sha.as_str(), whole.note.as_deref()), (prepared.merge_base.as_str(), None));
    let since = resolve(&prgit, &prepared, &BaseChoice::LastReview, Some(&commits[1]));
    assert_eq!((since.sha.as_str(), since.choice.clone(), since.note.clone()), (commits[1].as_str(), BaseChoice::LastReview, None));
    let files = prgit.files(&prepared, &since.sha).unwrap();
    assert_eq!(files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["src/b.rs"]);
    let any = resolve(&prgit, &prepared, &BaseChoice::Commit(commits[1].clone()), None);
    assert_eq!(any.sha, commits[1]);
}

#[test]
fn a_review_that_never_happened_or_is_gone_falls_back_to_the_whole_pull_request_and_says_why() {
    let (s, head, _) = setup();
    let prgit = s.prgit();
    let (_, pull) = s.pull_data(3, &head, &s.main_tip());
    let prepared = prgit.prepare(&pull).unwrap();
    let never = resolve(&prgit, &prepared, &BaseChoice::LastReview, None);
    assert_eq!((never.choice, never.sha.as_str()), (BaseChoice::Whole, prepared.merge_base.as_str()));
    assert!(never.note.unwrap().contains("not reviewed"));
    let stranger = resolve(&prgit, &prepared, &BaseChoice::LastReview, Some("0123456789abcdef0123456789abcdef01234567"));
    assert_eq!(stranger.choice, BaseChoice::Whole);
    assert!(stranger.note.unwrap().contains("not on this pull request"));
    let junk = resolve(&prgit, &prepared, &BaseChoice::Commit("--x".into()), None);
    assert_eq!(junk.choice, BaseChoice::Whole);
}

#[test]
fn a_rewritten_review_point_is_no_base() {
    let (s, head, _) = setup();
    let prgit = s.prgit();
    let base = s.main_tip();
    let (_, pull) = s.pull_data(3, &head, &base);
    prgit.prepare(&pull).unwrap();
    let new_head = s.force_push(3, "main", &|r: &Path| put(r, "src/a.rs", "rewritten\n"));
    let (_, moved) = s.pull_data(3, &new_head, &base);
    let prepared = prgit.prepare(&moved).unwrap();
    let base = resolve(&prgit, &prepared, &BaseChoice::LastReview, Some(&head));
    assert_eq!(base.choice, BaseChoice::Whole);
    assert!(base.note.unwrap().contains("rewritten"));
}

#[test]
fn the_head_as_the_base_says_nothing_has_been_pushed() {
    let (s, head, _) = setup();
    let prgit = s.prgit();
    let (_, pull) = s.pull_data(3, &head, &s.main_tip());
    let prepared = prgit.prepare(&pull).unwrap();
    let base = resolve(&prgit, &prepared, &BaseChoice::LastReview, Some(&head));
    assert_eq!((base.sha.as_str(), base.note.as_deref()), (head.as_str(), Some("Nothing has been pushed since.")));
    assert!(prgit.files(&prepared, &base.sha).unwrap().is_empty());
}
