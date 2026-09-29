//! The forge's data as the M0.4 models beui draws, so the UI needs no new types. The words follow
//! GitQuiet's (`docs/glossary.md`).
use beui::{
    court::{Court as UiCourt, CourtItem},
    merge::{
        MergeFacts, MergeMethod as UiMethod, PullState as UiPullState, Queue, ReviewNeed, Rights as UiRights,
        UpdateWay,
    },
    pr::{Checks, PrChipData, PrState, ReviewState},
};
use gpui_kit::SharedString;

use crate::{
    Check, CheckCounts, Conclusion, Court, Filed, MergeMethod, MergeState, Pull, PullBrief, PullState,
    ReviewDecision, Rights, Shelf, Verdict, time,
};

pub fn pr_state(state: PullState) -> PrState {
    match state {
        PullState::Open => PrState::Open,
        PullState::Draft => PrState::Draft,
        PullState::Merged => PrState::Merged,
        PullState::Closed => PrState::Closed,
    }
}

pub fn chip(brief: &PullBrief) -> PrChipData {
    PrChipData {
        number: brief.reference.number,
        repo: brief.reference.repo.slug().into(),
        title: brief.title.clone().into(),
        state: pr_state(brief.state),
        url: brief.url.clone().into(),
    }
}

pub fn checks(counts: CheckCounts) -> Checks {
    Checks { passed: counts.passed, failed: counts.failed, running: counts.running }
}

/// A review's standing as a row says it: asked, approved, or changes asked.
pub fn review_state(decision: ReviewDecision) -> ReviewState {
    match decision {
        ReviewDecision::NotRequired => ReviewState::None,
        ReviewDecision::Required => ReviewState::Requested,
        ReviewDecision::Approved => ReviewState::Approved,
        ReviewDecision::ChangesRequested => ReviewState::ChangesRequested,
    }
}

fn ui_court(court: Court) -> UiCourt {
    match court {
        Court::NeedsYou => UiCourt::NeedsYou,
        Court::Waiting => UiCourt::Waiting,
        Court::Running => UiCourt::Running,
        Court::Settled => UiCourt::Settled,
    }
}

/// Why a pull request sits in its Court, in a few words.
fn why(filed: &Filed) -> &'static str {
    let shelf = filed.involved.shelf;
    match filed.court {
        Court::Settled if filed.involved.summary.brief.state == PullState::Merged => "Merged",
        Court::Settled => "Closed",
        Court::Running if shelf == Some(Shelf::MergeQueue) => "In the merge queue",
        Court::Running => "Checks running",
        Court::NeedsYou => match shelf {
            Some(Shelf::NeedsAction) => "Needs your attention",
            Some(Shelf::TeamReviewRequested) => "Review asked of your team",
            Some(Shelf::YourDrafts) => "Your draft",
            _ => "Ready to merge",
        },
        Court::Waiting => match shelf {
            Some(Shelf::ReadyToMerge) => "Waiting on the pull request below",
            Some(Shelf::WaitingForReview) => "Waiting for review",
            None => "Involved",
            Some(_) => "Waiting",
        },
    }
}

/// One filed pull request as a row of the Court list. `now` is in epoch seconds.
pub fn court_item(filed: &Filed, now: u64) -> CourtItem {
    let summary = &filed.involved.summary;
    CourtItem {
        pr: chip(&summary.brief),
        author: SharedString::from(summary.author.clone()),
        court: ui_court(filed.court),
        why: why(filed).into(),
        checks: summary.checks.map(checks).unwrap_or_default(),
        review: review_state(summary.review),
        comments: summary.comments as usize,
        added: summary.additions as usize,
        removed: summary.deletions as usize,
        age: time::ago(now, summary.updated_at).into(),
        changed_at: summary.updated_at,
        unread: false,
    }
}

fn ui_method(method: MergeMethod) -> UiMethod {
    match method {
        MergeMethod::Merge => UiMethod::Merge,
        MergeMethod::Squash => UiMethod::Squash,
        MergeMethod::Rebase => UiMethod::Rebase,
    }
}

/// What merging needs, from the pull request. GitHub says whether a branch conflicts but not where, so
/// `conflicting_files` is the reader's own reading (a local `git merge-tree`); with none, the base
/// branch stands in as the one thing that conflicts. `checks` is the detailed list, when it is loaded:
/// with it, the required checks that fail or run are counted exactly. Without it they are taken from
/// the counts, and only when GitHub says a rule holds the merge up.
pub fn merge_facts(pull: &Pull, checks: Option<&[Check]>, conflicting_files: &[String]) -> MergeFacts {
    let (failing, running) = match checks {
        Some(list) => {
            let required = list.iter().filter(|c| c.required);
            let failing = required
                .clone()
                .filter(|c| matches!(c.conclusion, Some(Conclusion::Failure | Conclusion::TimedOut | Conclusion::Cancelled | Conclusion::ActionRequired | Conclusion::Stale)))
                .count();
            let running = required.filter(|c| c.conclusion.is_none()).count();
            (failing, running)
        }
        None if pull.merge_state == MergeState::Blocked => (pull.checks.failed as usize, pull.checks.running as usize),
        None => (0, 0),
    };
    let conflicts: Vec<SharedString> = if !pull.conflicting {
        Vec::new()
    } else if conflicting_files.is_empty() {
        vec![pull.base.clone().into()]
    } else {
        conflicting_files.iter().cloned().map(SharedString::from).collect()
    };
    let asked: Vec<SharedString> = pull
        .opinions
        .iter()
        .filter(|o| o.verdict == Verdict::RequestChanges)
        .map(|o| SharedString::from(o.reviewer.clone()))
        .collect();
    let review = match pull.review {
        ReviewDecision::Required => ReviewNeed::Missing,
        ReviewDecision::ChangesRequested => ReviewNeed::ChangesAsked(asked),
        ReviewDecision::Approved | ReviewDecision::NotRequired => ReviewNeed::Met,
    };
    MergeFacts {
        state: match pull.state {
            PullState::Merged => UiPullState::Merged,
            PullState::Closed => UiPullState::Closed,
            PullState::Open | PullState::Draft => UiPullState::Open,
        },
        draft: pull.state == PullState::Draft,
        conflicts,
        behind: (pull.merge_state == MergeState::Behind).then(|| vec![UpdateWay::Merge, UpdateWay::Rebase]),
        checks_failing: failing,
        checks_running: running,
        review,
        queue: (pull.merge.has_queue || pull.queue.is_some())
            .then(|| Queue { queued: pull.queue.is_some(), position: pull.queue.and_then(|q| q.position).map(|p| p as usize) }),
        methods: pull.merge.methods.iter().copied().map(ui_method).collect(),
        default_method: ui_method(pull.merge.default_method),
        auto_merge: pull.merge.auto_merge_allowed.then_some(pull.auto_merge),
        delete_branch: pull.merge.delete_branch_on_merge,
        rights: match pull.rights {
            Rights::Merge => UiRights::Merge,
            Rights::Bypass => UiRights::Bypass,
            Rights::Cannot => UiRights::Cannot,
        },
    }
}
