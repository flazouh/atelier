use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use super::Lookup;
use crate::{
    Forge, ForgeError, ForgeResult, Involved, MergeOutcome, PullBrief, PullRef, PullState, PullSummary, RepoRef,
    ReviewDecision,
};

fn current() -> RepoRef {
    RepoRef::new("github.com", "o", "r")
}

fn brief(repo: &RepoRef, number: u64) -> PullBrief {
    PullBrief {
        reference: PullRef { repo: repo.clone(), number },
        title: format!("PR {number}"),
        state: PullState::Open,
        url: format!("https://github.com/{}/pull/{number}", repo.slug()),
    }
}

/// A forge that knows some pull numbers of the current repository and some the reader is involved in,
/// and counts what it is asked.
#[derive(Default)]
struct Counting {
    here: Vec<u64>,
    involved: Vec<(RepoRef, u64)>,
    briefs_calls: Mutex<Vec<Vec<u64>>>,
    involved_calls: AtomicU64,
    fail: bool,
}

impl Forge for Counting {
    fn briefs(&self, repo: &RepoRef, numbers: &[u64]) -> ForgeResult<Vec<Option<PullBrief>>> {
        if self.fail {
            return Err(ForgeError::Offline);
        }
        self.briefs_calls.lock().unwrap().push(numbers.to_vec());
        Ok(numbers.iter().map(|n| self.here.contains(n).then(|| brief(repo, *n))).collect())
    }

    fn involved(&self) -> ForgeResult<Vec<Involved>> {
        self.involved_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .involved
            .iter()
            .map(|(repo, n)| Involved {
                summary: PullSummary {
                    brief: brief(repo, *n),
                    author: "a".into(),
                    created_at: 0,
                    updated_at: 0,
                    additions: 0,
                    deletions: 0,
                    comments: 0,
                    review: ReviewDecision::NotRequired,
                    checks: None,
                },
                shelf: None,
            })
            .collect())
    }

    fn repository(&self, _: &str) -> ForgeResult<crate::Repository> { unimplemented!() }
    fn pull(&self, _: &PullRef) -> ForgeResult<crate::Pull> { unimplemented!() }
    fn files(&self, _: &PullRef) -> ForgeResult<Vec<crate::ChangedFile>> { unimplemented!() }
    fn threads(&self, _: &PullRef) -> ForgeResult<Vec<crate::Thread>> { unimplemented!() }
    fn remarks(&self, _: &PullRef) -> ForgeResult<Vec<crate::Comment>> { unimplemented!() }
    fn checks(&self, _: &PullRef) -> ForgeResult<Vec<crate::Check>> { unimplemented!() }
    fn job(&self, _: &crate::JobRef) -> ForgeResult<crate::Job> { unimplemented!() }
    fn job_log(&self, _: &crate::JobRef) -> ForgeResult<String> { unimplemented!() }
    fn last_review_point(&self, _: &PullRef) -> ForgeResult<Option<String>> { unimplemented!() }
    fn create_pull(&self, _: &RepoRef, _: &crate::NewPull) -> ForgeResult<PullRef> { unimplemented!() }
    fn update_pull(&self, _: &PullRef, _: &crate::PullUpdate) -> ForgeResult<()> { unimplemented!() }
    fn merge(&self, _: &PullRef, _: &crate::MergeRequest) -> ForgeResult<MergeOutcome> { unimplemented!() }
    fn request_review(&self, _: &PullRef, _: &[crate::Reviewer]) -> ForgeResult<()> { unimplemented!() }
    fn comment(&self, _: &PullRef, _: &str) -> ForgeResult<crate::Comment> { unimplemented!() }
    fn hold_comment(&self, _: &PullRef, _: &crate::NewLine) -> ForgeResult<crate::HeldComment> { unimplemented!() }
    fn held_comments(&self, _: &PullRef) -> ForgeResult<Vec<crate::HeldComment>> { unimplemented!() }
    fn submit_review(&self, _: &PullRef, _: crate::Verdict, _: &str) -> ForgeResult<()> { unimplemented!() }
    fn reply(&self, _: &crate::ThreadId, _: &str) -> ForgeResult<crate::Comment> { unimplemented!() }
    fn resolve(&self, _: &crate::ThreadId, _: bool) -> ForgeResult<()> { unimplemented!() }
}

fn lookup(forge: &Arc<Counting>, clock: &Arc<AtomicU64>) -> Lookup {
    let clock = clock.clone();
    Lookup::new(forge.clone(), current()).with_clock(move || clock.load(Ordering::SeqCst))
}

#[test]
fn many_numbers_of_the_current_repository_are_one_request_in_order() {
    let forge = Arc::new(Counting { here: vec![1, 2, 3, 4], ..Counting::default() });
    let clock = Arc::new(AtomicU64::new(0));
    let found = lookup(&forge, &clock).resolve(&[3, 1, 4, 3]).unwrap();
    let numbers: Vec<_> = found.iter().map(|b| b.as_ref().unwrap().reference.number).collect();
    assert_eq!(numbers, [3, 1, 4, 3]);
    assert_eq!(*forge.briefs_calls.lock().unwrap(), [vec![1, 3, 4]], "one request, each number once");
    assert_eq!(forge.involved_calls.load(Ordering::SeqCst), 0, "everything was found at home");
}

#[test]
fn an_answer_stands_for_a_minute_and_then_is_asked_again() {
    let forge = Arc::new(Counting { here: vec![1], ..Counting::default() });
    let clock = Arc::new(AtomicU64::new(100));
    let lookup = lookup(&forge, &clock);
    lookup.resolve(&[1]).unwrap();
    clock.store(159, Ordering::SeqCst);
    lookup.resolve(&[1]).unwrap();
    assert_eq!(forge.briefs_calls.lock().unwrap().len(), 1, "still fresh at 59 seconds");
    clock.store(160, Ordering::SeqCst);
    lookup.resolve(&[1]).unwrap();
    assert_eq!(forge.briefs_calls.lock().unwrap().len(), 2);
}

#[test]
fn only_the_numbers_not_kept_are_asked_for() {
    let forge = Arc::new(Counting { here: vec![1, 2], ..Counting::default() });
    let clock = Arc::new(AtomicU64::new(0));
    let lookup = lookup(&forge, &clock);
    lookup.resolve(&[1]).unwrap();
    lookup.resolve(&[1, 2]).unwrap();
    assert_eq!(*forge.briefs_calls.lock().unwrap(), [vec![1], vec![2]]);
}

#[test]
fn a_number_that_is_not_a_pull_is_an_answer_that_is_kept_too() {
    let forge = Arc::new(Counting::default());
    let clock = Arc::new(AtomicU64::new(0));
    let lookup = lookup(&forge, &clock);
    assert_eq!(lookup.resolve(&[9]).unwrap(), [None]);
    lookup.resolve(&[9]).unwrap();
    assert_eq!(forge.briefs_calls.lock().unwrap().len(), 1, "the forge is not asked about the same issue again");
}

#[test]
fn a_number_not_at_home_is_found_among_the_pulls_the_reader_is_involved_in() {
    let other = RepoRef::new("github.com", "x", "y");
    let forge = Arc::new(Counting { here: vec![1], involved: vec![(other.clone(), 50)], ..Counting::default() });
    let clock = Arc::new(AtomicU64::new(0));
    let found = lookup(&forge, &clock).resolve(&[1, 50, 77]).unwrap();
    assert_eq!(found[0].as_ref().unwrap().reference.repo, current());
    assert_eq!(found[1].as_ref().unwrap().reference.repo, other);
    assert!(found[2].is_none());
}

#[test]
fn the_current_repository_wins_over_an_involved_one_with_the_same_number() {
    let other = RepoRef::new("github.com", "x", "y");
    let forge = Arc::new(Counting { here: vec![5], involved: vec![(other, 5)], ..Counting::default() });
    let clock = Arc::new(AtomicU64::new(0));
    let found = lookup(&forge, &clock).resolve(&[5]).unwrap();
    assert_eq!(found[0].as_ref().unwrap().reference.repo, current());
    assert_eq!(forge.involved_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn the_involved_list_is_read_once_for_five_minutes() {
    let forge = Arc::new(Counting::default());
    let clock = Arc::new(AtomicU64::new(0));
    let lookup = lookup(&forge, &clock);
    lookup.resolve(&[7]).unwrap();
    clock.store(299, Ordering::SeqCst);
    lookup.resolve(&[8]).unwrap();
    assert_eq!(forge.involved_calls.load(Ordering::SeqCst), 1);
    clock.store(300, Ordering::SeqCst);
    lookup.resolve(&[9]).unwrap();
    assert_eq!(forge.involved_calls.load(Ordering::SeqCst), 2);
}

#[test]
fn a_forge_that_fails_fails_the_lookup_and_keeps_nothing() {
    let failing = Arc::new(Counting { fail: true, ..Counting::default() });
    let clock = Arc::new(AtomicU64::new(0));
    assert_eq!(lookup(&failing, &clock).resolve(&[1]).err().unwrap(), ForgeError::Offline);
}

#[test]
fn no_numbers_ask_nothing() {
    let forge = Arc::new(Counting::default());
    let clock = Arc::new(AtomicU64::new(0));
    assert!(lookup(&forge, &clock).resolve(&[]).unwrap().is_empty());
    assert!(forge.briefs_calls.lock().unwrap().is_empty());
}
