//! GitQuiet's `workingSet.test.ts`, case for case, then the filing.
use crate::{
    CheckCounts, CheckState, Involved, PullBrief, PullRef, PullState, PullSummary, RepoRef, ReviewDecision, Shelf,
};

use super::{Court, Weighing, court_of, file};

fn weighing(shelf: Shelf) -> Weighing {
    Weighing {
        shelf: Some(shelf),
        state: PullState::Open,
        stands_on_unlanded: false,
        checks: None,
        review: ReviewDecision::NotRequired,
    }
}

fn stranger() -> Weighing {
    Weighing { shelf: None, ..weighing(Shelf::NeedsAction) }
}

#[test]
fn githubs_own_action_shelves_are_needs_you() {
    for shelf in [Shelf::NeedsAction, Shelf::ReadyToMerge, Shelf::TeamReviewRequested] {
        assert_eq!(court_of(&weighing(shelf)), Court::NeedsYou, "{shelf:?}");
    }
}

#[test]
fn a_draft_of_the_readers_own_is_theirs_to_finish() {
    assert_eq!(court_of(&weighing(Shelf::YourDrafts)), Court::NeedsYou);
}

#[test]
fn one_a_person_has_yet_to_answer_about_is_waiting() {
    let w = Weighing { review: ReviewDecision::Required, ..weighing(Shelf::WaitingForReview) };
    assert_eq!(court_of(&w), Court::Waiting);
}

#[test]
fn green_with_nobody_required_to_look_is_the_readers_to_land() {
    let w = Weighing { checks: Some(CheckState::Passing), ..weighing(Shelf::WaitingForReview) };
    assert_eq!(court_of(&w), Court::NeedsYou);
}

#[test]
fn green_but_standing_on_something_unlanded_is_still_waiting() {
    let w = Weighing { stands_on_unlanded: true, checks: Some(CheckState::Passing), ..weighing(Shelf::WaitingForReview) };
    assert_eq!(court_of(&w), Court::Waiting);
}

#[test]
fn checks_still_running_is_running_whoever_is_waiting_for_them() {
    let running = Some(CheckState::Running);
    assert_eq!(court_of(&Weighing { checks: running, ..weighing(Shelf::WaitingForReview) }), Court::Running);
    let approved = Weighing { checks: running, review: ReviewDecision::Approved, ..weighing(Shelf::ReadyToMerge) };
    assert_eq!(court_of(&approved), Court::Running);
}

#[test]
fn a_run_against_something_to_fix_is_still_the_readers_to_fix() {
    for shelf in [Shelf::NeedsAction, Shelf::TeamReviewRequested] {
        let w = Weighing { checks: Some(CheckState::Running), ..weighing(shelf) };
        assert_eq!(court_of(&w), Court::NeedsYou, "{shelf:?}");
    }
}

#[test]
fn one_the_forge_is_already_landing_is_running() {
    assert_eq!(court_of(&weighing(Shelf::MergeQueue)), Court::Running);
}

#[test]
fn before_the_checks_are_known_the_shelf_is_all_there_is() {
    assert_eq!(court_of(&weighing(Shelf::WaitingForReview)), Court::Waiting);
}

#[test]
fn merged_and_closed_are_settled_whatever_shelf_they_arrived_on() {
    for shelf in Shelf::ALL {
        for state in [PullState::Merged, PullState::Closed] {
            assert_eq!(court_of(&Weighing { state, ..weighing(shelf) }), Court::Settled, "{shelf:?} {state:?}");
        }
    }
}

#[test]
fn one_ready_to_land_above_a_pull_request_that_has_not_landed_is_waiting() {
    let w = Weighing { stands_on_unlanded: true, ..weighing(Shelf::ReadyToMerge) };
    assert_eq!(court_of(&w), Court::Waiting);
}

#[test]
fn a_stack_does_not_excuse_the_reader_from_what_is_broken() {
    for shelf in [Shelf::NeedsAction, Shelf::YourDrafts] {
        assert_eq!(court_of(&Weighing { stands_on_unlanded: true, ..weighing(shelf) }), Court::NeedsYou);
    }
}

#[test]
fn one_on_none_of_the_shelves_is_waiting_and_settled_once_merged_or_closed() {
    assert_eq!(court_of(&stranger()), Court::Waiting);
    assert_eq!(court_of(&Weighing { state: PullState::Merged, ..stranger() }), Court::Settled);
    assert_eq!(court_of(&Weighing { state: PullState::Closed, ..stranger() }), Court::Settled);
}

fn involved(number: u64, shelf: Shelf, updated_at: u64) -> Involved {
    Involved {
        summary: PullSummary {
            brief: PullBrief {
                reference: PullRef { repo: RepoRef::new("github.com", "o", "r"), number },
                title: format!("PR {number}"),
                state: PullState::Open,
                url: String::new(),
            },
            author: "a".into(),
            created_at: 0,
            updated_at,
            additions: 0,
            deletions: 0,
            comments: 0,
            review: ReviewDecision::Required,
            checks: Some(CheckCounts { passed: 1, failed: 0, running: 0 }),
            standing: Default::default(),
        },
        shelf: Some(shelf),
    }
}

#[test]
fn filing_puts_courts_in_reading_order_newest_first_and_leaves_empty_ones_out() {
    let filed = file(
        vec![
            involved(1, Shelf::WaitingForReview, 10),
            involved(2, Shelf::NeedsAction, 5),
            involved(3, Shelf::NeedsAction, 50),
            involved(4, Shelf::MergeQueue, 1),
        ],
        |_| false,
    );
    let shape: Vec<_> = filed
        .iter()
        .map(|(court, rows)| (*court, rows.iter().map(|r| r.involved.summary.brief.reference.number).collect::<Vec<_>>()))
        .collect();
    assert_eq!(
        shape,
        [(Court::NeedsYou, vec![3, 2]), (Court::Waiting, vec![1]), (Court::Running, vec![4])]
    );
}

#[test]
fn a_pull_request_on_two_shelves_is_kept_once_in_its_most_urgent_court() {
    let filed = file(vec![involved(1, Shelf::WaitingForReview, 10), involved(1, Shelf::NeedsAction, 10)], |_| false);
    assert_eq!(filed.len(), 1);
    assert_eq!(filed[0].0, Court::NeedsYou);
    assert_eq!(filed[0].1.len(), 1);
    let again = file(vec![involved(1, Shelf::NeedsAction, 10), involved(1, Shelf::WaitingForReview, 10)], |_| false);
    assert_eq!(again[0].0, Court::NeedsYou);
}

#[test]
fn a_stack_demotes_a_ready_pull_request_through_the_lookup() {
    let filed = file(vec![involved(7, Shelf::ReadyToMerge, 1)], |r| r.number == 7);
    assert_eq!(filed[0].0, Court::Waiting);
}
