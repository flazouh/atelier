use std::{sync::Mutex, time::Duration};

use atelier_forge::{Change, ChangedFile, ForgeError, PullState};

use crate::{
    Part, PartKind, PullData,
    fixture::{FixtureForge, sample},
    sync::{Cadence, Delta, delta, keep_place, refresh},
};

const S: fn(u64) -> Duration = Duration::from_secs;

#[test]
fn asking_slows_while_nothing_happens_and_starts_over_when_something_does() {
    let mut cadence = Cadence::new(S(30));
    assert_eq!(cadence.after_answer(true), S(30));
    let waits: Vec<_> = (0..8).map(|_| cadence.after_answer(false)).collect();
    assert_eq!(waits, [S(30), S(60), S(60), S(90), S(90), S(120), S(120), S(120)], "up to four times the base, never more");
    assert_eq!(cadence.after_answer(true), S(30));
}

#[test]
fn a_failure_backs_off_by_doubling_and_an_answer_forgets_it() {
    let mut cadence = Cadence::new(S(30));
    assert_eq!(cadence.after_failure(&ForgeError::Offline), Some(S(60)));
    assert_eq!(cadence.after_failure(&ForgeError::Offline), Some(S(120)));
    assert_eq!(cadence.after_failure(&ForgeError::Unexpected("x".into())), Some(S(240)));
    assert_eq!(cadence.after_failure(&ForgeError::Offline), Some(S(300)), "five minutes at most");
    assert_eq!(cadence.after_failure(&ForgeError::Offline), Some(S(300)));
    cadence.after_answer(false);
    assert_eq!(cadence.after_failure(&ForgeError::Offline), Some(S(60)), "the count started over");
}

#[test]
fn a_rate_limit_is_obeyed_up_to_an_hour() {
    let mut cadence = Cadence::new(S(30));
    assert_eq!(cadence.after_failure(&ForgeError::RateLimited { retry_after: Some(600) }), Some(S(601)));
    assert_eq!(cadence.after_failure(&ForgeError::RateLimited { retry_after: Some(99_999) }), Some(S(3600)));
    let mut cadence = Cadence::new(S(30));
    assert_eq!(cadence.after_failure(&ForgeError::RateLimited { retry_after: None }), Some(S(60)));
}

#[test]
fn a_failure_that_waiting_cannot_cure_stops_the_asking() {
    let mut cadence = Cadence::new(S(30));
    for error in [ForgeError::NotSignedIn, ForgeError::ToolMissing { tool: "gh".into() }, ForgeError::Denied("x".into()), ForgeError::NotFound("x".into()), ForgeError::UnknownRemote("x".into())] {
        assert_eq!(cadence.after_failure(&error), None, "{error:?}");
    }
}

fn known() -> (FixtureForge, PullData) {
    let data = sample::data(5, "head1", vec![ChangedFile { path: "a.rs".into(), additions: 1, deletions: 0, change: Change::Modified }]);
    (FixtureForge::new().with_pull(data.clone()), data)
}

fn run(forge: &FixtureForge, known: &PullData) -> (Result<crate::sync::Refreshed, ForgeError>, Vec<Part>) {
    let parts = Mutex::new(Vec::new());
    let result = refresh(forge, &known.reference, known, &|p| parts.lock().unwrap().push(p));
    (result, parts.into_inner().unwrap())
}

#[test]
fn a_quiet_pull_request_costs_one_call() {
    let (forge, known) = known();
    let (result, parts) = run(&forge, &known);
    assert_eq!(result.unwrap(), Default::default());
    assert!(parts.is_empty());
    assert_eq!(forge.calls(), vec!["pull"]);
}

#[test]
fn a_new_comment_rereads_what_people_change_but_not_the_files() {
    let (forge, known) = known();
    forge.edit(&known.reference, |d| {
        d.pull.as_mut().unwrap().updated_at += 60;
        d.threads.push(sample::thread("T9", "a.rs", 1, vec![sample::comment("c9", "Ada", "Hm", 5)]));
    });
    let (result, parts) = run(&forge, &known);
    let result = result.unwrap();
    assert!(result.changed && !result.head_moved);
    let kinds: Vec<_> = parts.iter().map(|p| match p { Part::Pull(_) => "pull", Part::Threads(_) => "threads", Part::Remarks(_) => "remarks", Part::Checks(_) => "checks", Part::Held(_) => "held", Part::Files(_) => "files", Part::ReviewPoint(_) => "point", Part::Failed { .. } => "failed" }).collect();
    assert!(kinds.contains(&"pull") && kinds.contains(&"threads") && !kinds.contains(&"files") && !kinds.contains(&"point"), "{kinds:?}");
    let mut fresh = known.clone();
    for part in parts {
        fresh.apply(part, 9);
    }
    assert_eq!(fresh.threads.len(), 1);
}

#[test]
fn a_push_rereads_the_files_and_the_review_point_too() {
    let (forge, known) = known();
    forge.edit(&known.reference, |d| d.pull.as_mut().unwrap().head_sha = "head2".into());
    let (result, parts) = run(&forge, &known);
    assert!(result.unwrap().head_moved);
    assert!(parts.iter().any(|p| matches!(p, Part::Files(_))) && parts.iter().any(|p| matches!(p, Part::ReviewPoint(_))));
}

#[test]
fn a_header_that_cannot_be_read_is_the_error_and_nothing_else_is_asked() {
    let (forge, known) = known();
    forge.fail("pull", ForgeError::Offline);
    let (result, parts) = run(&forge, &known);
    assert_eq!(result.unwrap_err(), ForgeError::Offline);
    assert!(parts.is_empty());
    assert_eq!(forge.calls(), vec!["pull"]);
}

#[test]
fn a_part_that_fails_in_a_refresh_is_reported_and_the_rest_arrive() {
    let (forge, known) = known();
    forge.edit(&known.reference, |d| d.pull.as_mut().unwrap().updated_at += 1);
    forge.fail("checks", ForgeError::Unexpected("bad".into()));
    let (result, parts) = run(&forge, &known);
    assert!(result.unwrap().changed);
    assert!(parts.iter().any(|p| matches!(p, Part::Failed { part: PartKind::Checks, .. })));
    assert!(parts.iter().any(|p| matches!(p, Part::Threads(_))));
}

#[test]
fn the_delta_names_what_changed() {
    let (_, old) = known();
    let mut new = old.clone();
    assert!(delta(&old, &new).is_empty());
    assert_eq!(delta(&old, &new).words(), None);
    new.pull.as_mut().unwrap().head_sha = "head2".into();
    new.threads.push(sample::thread("T1", "a.rs", 1, vec![sample::comment("c1", "Ada", "a", 1), sample::comment("c2", "Rui", "b", 2)]));
    new.remarks.push(sample::comment("r1", "Bot", "c", 3));
    new.pull.as_mut().unwrap().state = PullState::Merged;
    let d = delta(&old, &new);
    assert_eq!((d.head_moved, d.new_comments, d.state), (true, 3, Some(PullState::Merged)));
    assert_eq!(d.words().as_deref(), Some("A new push, 3 new comments, merged"));
    assert_eq!(Delta { new_comments: 1, ..Delta::default() }.words().as_deref(), Some("1 new comment"));
}

#[test]
fn the_reader_keeps_their_place_when_the_files_change() {
    let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let before = names(&["a", "b", "c", "d"]);
    assert_eq!(keep_place(Some("b"), &before, &names(&["a", "b", "x"])).as_deref(), Some("b"), "still there");
    assert_eq!(keep_place(Some("c"), &before, &names(&["a", "b", "d"])).as_deref(), Some("d"), "the file that stands where it stood");
    assert_eq!(keep_place(Some("d"), &before, &names(&["a", "b"])).as_deref(), Some("b"), "or the last, when the list got shorter");
    assert_eq!(keep_place(None, &before, &names(&["a"])), None, "no place to keep");
    assert_eq!(keep_place(Some("a"), &before, &[]), None);
    assert_eq!(keep_place(Some("zz"), &before, &names(&["p", "q"])).as_deref(), Some("p"), "a file never in the list starts at the first");
}
