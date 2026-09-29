//! Who owes the next move on a pull request, after GitQuiet's working set (`src/domain/workingSet.ts`,
//! same rules, same tests). The forge says which shelf holds a pull request; the Court follows from the
//! shelf, the state, the checks and the review.
use std::collections::HashMap;

use crate::{
    CheckState, Involved, PullRef, PullState, ReviewDecision, Shelf,
};

/// Who owes the next move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Court {
    NeedsYou,
    Waiting,
    Running,
    Settled,
}

impl Court {
    /// Reading order, most urgent first.
    pub const ALL: [Court; 4] = [Self::NeedsYou, Self::Waiting, Self::Running, Self::Settled];

    fn urgency(self) -> usize {
        Self::ALL.iter().position(|court| *court == self).unwrap_or(usize::MAX)
    }
}

/// What decides a Court.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weighing {
    /// `None` for a pull request on none of the reader's shelves.
    pub shelf: Option<Shelf>,
    pub state: PullState,
    /// The pull request stands on another that has not landed.
    pub stands_on_unlanded: bool,
    /// `None` until the checks are known.
    pub checks: Option<CheckState>,
    pub review: ReviewDecision,
}

/// The two shelves that hold nothing but a wait: a machine's run can demote them, and a green one
/// with no review required is the reader's to land.
fn nothing_but_the_run(shelf: Shelf) -> bool {
    matches!(shelf, Shelf::WaitingForReview | Shelf::ReadyToMerge)
}

fn court_of_shelf(shelf: Shelf) -> Court {
    match shelf {
        Shelf::NeedsAction | Shelf::TeamReviewRequested | Shelf::ReadyToMerge | Shelf::YourDrafts => Court::NeedsYou,
        Shelf::WaitingForReview => Court::Waiting,
        Shelf::MergeQueue => Court::Running,
    }
}

pub fn court_of(weighing: &Weighing) -> Court {
    let Weighing { shelf, state, stands_on_unlanded, checks, review } = *weighing;
    if matches!(state, PullState::Merged | PullState::Closed) {
        return Court::Settled;
    }
    let Some(shelf) = shelf else { return Court::Waiting };
    if shelf == Shelf::ReadyToMerge && stands_on_unlanded {
        return Court::Waiting;
    }
    if nothing_but_the_run(shelf) {
        if checks == Some(CheckState::Running) {
            return Court::Running;
        }
        if !stands_on_unlanded && checks == Some(CheckState::Passing) && review == ReviewDecision::NotRequired {
            return Court::NeedsYou;
        }
    }
    court_of_shelf(shelf)
}

/// A pull request with its Court.
#[derive(Clone, Debug, PartialEq)]
pub struct Filed {
    pub involved: Involved,
    pub court: Court,
}

/// The working set filed into Courts: reading order, empty Courts left out, each pull request once in
/// its most urgent Court, and inside a Court the newest change first. `stands_on_unlanded` says whether
/// a pull request stands on another that has not landed.
pub fn file(
    involved: Vec<Involved>,
    stands_on_unlanded: impl Fn(&PullRef) -> bool,
) -> Vec<(Court, Vec<Filed>)> {
    let mut once: HashMap<PullRef, Filed> = HashMap::with_capacity(involved.len());
    let mut order: Vec<PullRef> = Vec::new();
    for item in involved {
        let reference = item.summary.brief.reference.clone();
        let court = court_of(&Weighing {
            shelf: item.shelf,
            state: item.summary.brief.state,
            stands_on_unlanded: stands_on_unlanded(&reference),
            checks: item.summary.checks.and_then(|counts| counts.state()),
            review: item.summary.review,
        });
        match once.get_mut(&reference) {
            Some(kept) if court.urgency() < kept.court.urgency() => *kept = Filed { involved: item, court },
            Some(_) => {}
            None => {
                order.push(reference.clone());
                once.insert(reference, Filed { involved: item, court });
            }
        }
    }
    Court::ALL
        .into_iter()
        .filter_map(|court| {
            let mut rows: Vec<Filed> =
                order.iter().filter_map(|r| once.get(r)).filter(|f| f.court == court).cloned().collect();
            rows.sort_by_key(|row| std::cmp::Reverse(row.involved.summary.updated_at));
            (!rows.is_empty()).then_some((court, rows))
        })
        .collect()
}

#[cfg(test)]
mod tests;
