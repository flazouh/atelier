use lathe_forge::{CheckCounts, Involved, PullBrief, PullState, PullSummary, ReviewDecision, Shelf};

use crate::{
    fixture::sample,
    list::ListModel,
    snapshot::ListSnapshot,
};

fn involved(number: u64, state: PullState, shelf: Option<Shelf>, updated: u64) -> Involved {
    let reference = sample::reference(number);
    Involved {
        summary: PullSummary {
            brief: PullBrief { reference: reference.clone(), title: format!("Pull {number}"), state, url: format!("https://github.com/flazouh/relay/pull/{number}") },
            author: "Rui".into(),
            created_at: updated - 100,
            updated_at: updated,
            additions: 10,
            deletions: 2,
            comments: 3,
            review: ReviewDecision::Required,
            checks: Some(CheckCounts { passed: 4, failed: 0, running: 0 }),
        },
        shelf,
    }
}

#[test]
fn the_working_set_is_filed_by_who_owes_the_next_move() {
    let mut list = ListModel::default();
    assert!(!list.loaded && list.is_empty());
    list.set(
        vec![
            involved(1, PullState::Open, Some(Shelf::NeedsAction), 900),
            involved(2, PullState::Open, Some(Shelf::WaitingForReview), 800),
            involved(3, PullState::Merged, None, 700),
            involved(4, PullState::Open, Some(Shelf::NeedsAction), 950),
        ],
        1000,
    );
    assert!(list.loaded);
    assert_eq!((list.len(), list.needs_you()), (4, 2));
    let rows = list.rows(sample::NOW);
    assert_eq!(rows.iter().map(|r| r.pr.number).collect::<Vec<_>>(), [4, 1, 2, 3], "Needs you first, the newest change first inside it, settled last");
    assert_eq!(rows[0].why.as_ref(), "Needs your attention");
    assert_eq!(rows[0].checks.passed, 4);
}

#[test]
fn a_row_is_unread_until_the_reader_opens_it_after_its_last_change() {
    let mut list = ListModel::default();
    list.set(vec![involved(1, PullState::Open, Some(Shelf::NeedsAction), 900)], 1000);
    assert!(list.rows(sample::NOW)[0].unread, "never opened");
    list.mark_opened(&sample::reference(1), 950);
    assert!(!list.rows(sample::NOW)[0].unread);
    list.set(vec![involved(1, PullState::Open, Some(Shelf::NeedsAction), 990)], 1100);
    assert!(list.rows(sample::NOW)[0].unread, "changed since it was opened");
}

#[test]
fn a_row_finds_its_pull_request_again() {
    let mut list = ListModel::default();
    list.set(vec![involved(7, PullState::Open, Some(Shelf::NeedsAction), 900)], 1000);
    let chip = &list.rows(sample::NOW)[0].pr;
    assert_eq!(list.reference_of(chip), Some(sample::reference(7)));
    let mut other = chip.clone();
    other.number = 8;
    assert_eq!(list.reference_of(&other), None);
}

#[test]
fn only_open_and_draft_pull_requests_of_a_repository_keep_their_checkout() {
    let mut list = ListModel::default();
    list.set(vec![involved(1, PullState::Open, None, 900), involved(2, PullState::Draft, None, 900), involved(3, PullState::Merged, None, 900), involved(4, PullState::Closed, None, 900)], 1000);
    let mut open = list.open_numbers(&sample::reference(1).repo);
    open.sort();
    assert_eq!(open, vec![1, 2]);
    assert!(list.open_numbers(&lathe_forge::RepoRef::new("github.com", "x", "y")).is_empty());
}

#[test]
fn the_list_comes_back_from_disk() {
    let dir = tempfile::tempdir().unwrap();
    let snapshot = ListSnapshot::new(dir.path().join("pr"));
    assert!(snapshot.load().is_none());
    let items = vec![involved(1, PullState::Open, Some(Shelf::NeedsAction), 900)];
    snapshot.save(&items, 1234).unwrap();
    assert_eq!(snapshot.load(), Some((items, 1234)));
    std::fs::write(dir.path().join("pr/involved.json"), "{broken").unwrap();
    assert!(snapshot.load().is_none(), "a damaged file is a miss");
}
