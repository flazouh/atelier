use std::sync::Mutex;

use lathe_forge::{Change, ChangedFile, CheckStatus, Conclusion, Forge, ForgeError};

use crate::{
    Part, PartKind, PullData,
    fixture::{FixtureForge, Write, sample},
    load::{LIVE, load_all, load_parts},
};

fn files() -> Vec<ChangedFile> {
    vec![ChangedFile { path: "src/a.rs".into(), additions: 3, deletions: 1, change: Change::Modified }]
}

fn forge() -> (FixtureForge, lathe_forge::PullRef) {
    let mut data = sample::data(7, "abc123", files());
    data.checks = vec![sample::check("linux", CheckStatus::Done, Some(Conclusion::Success))];
    data.threads = vec![sample::thread("T1", "src/a.rs", 2, vec![sample::comment("c1", "Ada", "Why?", 100)])];
    data.remarks = vec![sample::comment("r1", "Bot", "Built.", 101)];
    let reference = data.reference.clone();
    (FixtureForge::new().with_pull(data), reference)
}

fn collect(forge: &dyn Forge, reference: &lathe_forge::PullRef, kinds: &[PartKind]) -> Vec<Part> {
    let parts = Mutex::new(Vec::new());
    load_parts(forge, reference, kinds, &|p| parts.lock().unwrap().push(p));
    parts.into_inner().unwrap()
}

#[test]
fn every_part_arrives_and_builds_the_pull_request() {
    let (forge, reference) = forge();
    let parts = Mutex::new(Vec::new());
    load_all(&forge, &reference, &|p| parts.lock().unwrap().push(p));
    let parts = parts.into_inner().unwrap();
    assert_eq!(parts.len(), 7);
    let mut data = PullData::new(reference);
    assert!(!data.has(PartKind::Pull) && !data.complete());
    for part in parts {
        data.apply(part, 500);
    }
    assert!(data.complete());
    assert_eq!(data.fetched_at, 500);
    assert_eq!(data.pull.as_ref().unwrap().head_sha, "abc123");
    assert_eq!((data.files.len(), data.threads.len(), data.remarks.len(), data.checks.len()), (1, 1, 1, 1));
    assert_eq!(data.comment_count(), 2);
}

#[test]
fn a_part_that_fails_is_reported_and_the_others_still_arrive() {
    let (forge, reference) = forge();
    forge.fail("threads", ForgeError::Offline);
    let parts = collect(&forge, &reference, &LIVE);
    assert_eq!(parts.len(), 5);
    assert!(parts.iter().any(|p| matches!(p, Part::Failed { part: PartKind::Threads, error: ForgeError::Offline })));
    let mut data = sample::data(7, "old", files());
    let before = data.threads.clone();
    for part in parts {
        data.apply(part, 9);
    }
    assert_eq!(data.pull.as_ref().unwrap().head_sha, "abc123", "the others were taken in");
    assert_eq!(data.threads, before, "a failed part leaves what was there");
}

#[test]
fn a_refresh_reads_only_the_parts_that_change_while_people_work() {
    let (forge, reference) = forge();
    collect(&forge, &reference, &LIVE);
    let calls = forge.calls();
    assert!(!calls.contains(&"files") && !calls.contains(&"last_review_point"), "{calls:?}");
    assert_eq!(calls.len(), 5);
}

#[test]
fn the_reads_run_at_once_not_one_after_another() {
    // Each call takes a moment; seven of them in a row would take seven moments.
    struct Slow(FixtureForge);
    macro_rules! slow {
        ($self:ident, $name:ident($($arg:ident: $ty:ty),*) -> $ret:ty) => {
            fn $name(&$self, $($arg: $ty),*) -> $ret {
                std::thread::sleep(std::time::Duration::from_millis(100));
                $self.0.$name($($arg),*)
            }
        };
    }
    impl Forge for Slow {
        slow!(self, repository(u: &str) -> lathe_forge::ForgeResult<lathe_forge::Repository>);
        slow!(self, pull(r: &lathe_forge::PullRef) -> lathe_forge::ForgeResult<lathe_forge::Pull>);
        slow!(self, files(r: &lathe_forge::PullRef) -> lathe_forge::ForgeResult<Vec<ChangedFile>>);
        slow!(self, threads(r: &lathe_forge::PullRef) -> lathe_forge::ForgeResult<Vec<lathe_forge::Thread>>);
        slow!(self, remarks(r: &lathe_forge::PullRef) -> lathe_forge::ForgeResult<Vec<lathe_forge::Remark>>);
        slow!(self, checks(r: &lathe_forge::PullRef) -> lathe_forge::ForgeResult<Vec<lathe_forge::Check>>);
        slow!(self, job(j: &lathe_forge::JobRef) -> lathe_forge::ForgeResult<lathe_forge::Job>);
        slow!(self, job_log(j: &lathe_forge::JobRef) -> lathe_forge::ForgeResult<String>);
        slow!(self, last_review_point(r: &lathe_forge::PullRef) -> lathe_forge::ForgeResult<Option<String>>);
        slow!(self, involved() -> lathe_forge::ForgeResult<Vec<lathe_forge::Involved>>);
        slow!(self, briefs(r: &lathe_forge::RepoRef, n: &[u64]) -> lathe_forge::ForgeResult<Vec<Option<lathe_forge::PullBrief>>>);
        slow!(self, create_pull(r: &lathe_forge::RepoRef, n: &lathe_forge::NewPull) -> lathe_forge::ForgeResult<lathe_forge::PullRef>);
        slow!(self, update_pull(r: &lathe_forge::PullRef, u: &lathe_forge::PullUpdate) -> lathe_forge::ForgeResult<()>);
        slow!(self, merge(r: &lathe_forge::PullRef, m: &lathe_forge::MergeRequest) -> lathe_forge::ForgeResult<lathe_forge::MergeOutcome>);
        slow!(self, request_review(r: &lathe_forge::PullRef, v: &[lathe_forge::Reviewer]) -> lathe_forge::ForgeResult<()>);
        slow!(self, comment(r: &lathe_forge::PullRef, b: &str) -> lathe_forge::ForgeResult<lathe_forge::Comment>);
        slow!(self, hold_comment(r: &lathe_forge::PullRef, c: &lathe_forge::NewLine) -> lathe_forge::ForgeResult<lathe_forge::HeldComment>);
        slow!(self, held_comments(r: &lathe_forge::PullRef) -> lathe_forge::ForgeResult<Vec<lathe_forge::HeldComment>>);
        slow!(self, submit_review(r: &lathe_forge::PullRef, v: lathe_forge::Verdict, b: &str) -> lathe_forge::ForgeResult<()>);
        slow!(self, reply(t: &lathe_forge::ThreadId, b: &str) -> lathe_forge::ForgeResult<lathe_forge::Comment>);
        slow!(self, resolve(t: &lathe_forge::ThreadId, r: bool) -> lathe_forge::ForgeResult<()>);
    }
    let (inner, reference) = forge();
    let slow = Slow(inner);
    let started = std::time::Instant::now();
    let parts = collect(&slow, &reference, &PartKind::ALL);
    assert_eq!(parts.len(), 7);
    assert!(started.elapsed() < std::time::Duration::from_millis(400), "{:?}", started.elapsed());
}

#[test]
fn writes_change_what_the_fixture_reads_back() {
    let (forge, reference) = forge();
    forge.reply(&lathe_forge::ThreadId("T1".into()), "Because.").unwrap();
    forge.resolve(&lathe_forge::ThreadId("T1".into()), true).unwrap();
    forge.comment(&reference, "Thanks").unwrap();
    let data = forge.data(&reference).unwrap();
    assert_eq!(data.threads[0].comments.len(), 2);
    assert!(data.threads[0].resolved);
    assert_eq!(data.remarks.len(), 2);
    assert_eq!(forge.writes().len(), 3);
    assert!(matches!(forge.writes()[0], Write::Reply { .. }));
    assert!(forge.reply(&lathe_forge::ThreadId("nope".into()), "x").is_err());
}
