use super::{Rule, RuleSet, Signal};
use crate::{PrLink, SessionLink, Status, TaskId};

fn started() -> Signal {
    Signal::SessionStarted {
        task: TaskId::from("1"),
        session: SessionLink { session_id: "s1".into(), title: "Do it".into(), agent: "Claude".into() },
    }
}

fn finished(ok: bool) -> Signal {
    Signal::SessionFinished { session_id: "s1".into(), ok }
}

fn merged() -> Signal {
    Signal::PrMerged { number: 7, by: "alex".into() }
}

fn to(rules: &RuleSet, status: Status, signal: &Signal) -> Option<Status> {
    rules.decide(status, signal).map(|d| d.to)
}

#[test]
fn a_session_started_moves_a_task_that_waits_to_in_progress() {
    let rules = RuleSet::default();
    for status in [Status::Backlog, Status::Todo] {
        assert_eq!(to(&rules, status, &started()), Some(Status::InProgress), "{status:?}");
    }
    for status in [Status::InProgress, Status::InReview, Status::Done, Status::Canceled] {
        assert_eq!(to(&rules, status, &started()), None, "{status:?} stays");
    }
}

#[test]
fn the_agent_finishing_moves_in_progress_to_in_review_and_a_failure_moves_nothing() {
    let rules = RuleSet::default();
    assert_eq!(to(&rules, Status::InProgress, &finished(true)), Some(Status::InReview));
    assert_eq!(to(&rules, Status::InProgress, &finished(false)), None, "a failed session is not a finished task");
    for status in [Status::Backlog, Status::Todo, Status::InReview, Status::Done, Status::Canceled] {
        assert_eq!(to(&rules, status, &finished(true)), None, "{status:?}");
    }
}

#[test]
fn a_merge_moves_every_open_task_to_done_and_leaves_a_closed_one() {
    let rules = RuleSet::default();
    for status in [Status::Backlog, Status::Todo, Status::InProgress, Status::InReview] {
        assert_eq!(to(&rules, status, &merged()), Some(Status::Done), "{status:?}");
    }
    assert_eq!(to(&rules, Status::Done, &merged()), None);
    assert_eq!(to(&rules, Status::Canceled, &merged()), None, "a canceled task is not brought back");
}

#[test]
fn opening_a_pull_request_moves_nothing() {
    let signal = Signal::PrOpened { task: TaskId::from("1"), pr: PrLink { number: 7, repo: "o/r".into() }, by: "x".into() };
    for status in Status::ALL {
        assert_eq!(to(&RuleSet::default(), status, &signal), None);
    }
}

#[test]
fn a_rule_turned_off_does_nothing_and_the_others_still_work() {
    let mut rules = RuleSet::default();
    rules.set(Rule::MergeMovesToDone, false);
    assert!(!rules.is_on(Rule::MergeMovesToDone));
    assert_eq!(to(&rules, Status::InReview, &merged()), None);
    assert_eq!(to(&rules, Status::Todo, &started()), Some(Status::InProgress));
    rules.set(Rule::MergeMovesToDone, true);
    assert_eq!(to(&rules, Status::InReview, &merged()), Some(Status::Done));
}

#[test]
fn the_decision_names_its_rule() {
    let decision = RuleSet::default().decide(Status::Todo, &started()).unwrap();
    assert_eq!(decision.rule, Rule::SessionStartMovesToInProgress);
    assert_eq!(decision.rule.id(), "session-start");
}

#[test]
fn the_switches_round_trip_through_the_settings_and_skip_unknown_ids() {
    let mut rules = RuleSet::default();
    rules.set(Rule::AgentFinishMovesToInReview, false);
    assert_eq!(rules.disabled(), vec!["agent-finish"]);
    assert_eq!(RuleSet::from_disabled(rules.disabled()), rules);
    assert_eq!(RuleSet::from_disabled(["agent-finish", "a-rule-from-the-future"]), rules);
    assert_eq!(RuleSet::from_disabled([]), RuleSet::default());
}

#[test]
fn every_rule_has_a_unique_id_and_a_sentence() {
    let mut ids: Vec<_> = Rule::ALL.iter().map(|r| r.id()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), Rule::ALL.len());
    assert!(Rule::ALL.iter().all(|r| !r.words().is_empty() && Rule::from_id(r.id()) == Some(*r)));
}

mod flow {
    use super::*;
    use crate::{ActivityKind, Entry, LocalTracker, NewTask, Tracker, handle};

    fn tracker() -> LocalTracker {
        LocalTracker::in_memory("LAT").unwrap()
    }

    fn session(id: &str) -> SessionLink {
        SessionLink { session_id: id.into(), title: "Work".into(), agent: "Claude".into() }
    }

    fn pr(number: u64) -> PrLink {
        PrLink { number, repo: "o/r".into() }
    }

    fn kinds(t: &LocalTracker, id: &TaskId) -> Vec<(String, ActivityKind)> {
        t.activity(id).unwrap().into_iter().map(|a| (a.by, a.kind)).collect()
    }

    #[test]
    fn a_task_goes_from_todo_to_done_as_its_session_and_pull_request_go() {
        let t = tracker();
        let rules = RuleSet::default();
        let task = t.create(&NewTask::titled("Ship it"), "alex").unwrap();
        let id = task.id.clone();

        let started = handle(&t, &rules, &Signal::SessionStarted { task: id.clone(), session: session("s1") }).unwrap();
        assert_eq!(started[0].task.status, Status::InProgress);
        assert_eq!(started[0].moved.map(|d| d.rule), Some(Rule::SessionStartMovesToInProgress));
        assert_eq!(started[0].task.sessions, vec![session("s1")], "the session is linked");

        let finished = handle(&t, &rules, &finished(true)).unwrap();
        assert_eq!(finished[0].task.status, Status::InReview);

        let opened = handle(&t, &rules, &Signal::PrOpened { task: id.clone(), pr: pr(7), by: "Claude".into() }).unwrap();
        assert_eq!((opened[0].task.status, opened[0].moved), (Status::InReview, None), "opening moves nothing");
        assert_eq!(opened[0].task.prs, vec![pr(7)]);

        let done = handle(&t, &rules, &merged()).unwrap();
        assert_eq!(done[0].task.status, Status::Done);

        let log = kinds(&t, &id);
        let moves: Vec<_> = log.iter().filter(|(_, k)| matches!(k, ActivityKind::StatusChanged { .. })).collect();
        assert_eq!(
            moves.iter().map(|(by, _)| by.as_str()).collect::<Vec<_>>(),
            ["rule:session-start", "rule:agent-finish", "rule:merge"],
            "the log says which rule moved the task"
        );
        assert!(log.iter().any(|(by, k)| by == "Claude" && matches!(k, ActivityKind::SessionStarted { .. })));
        assert!(log.iter().any(|(by, k)| by == "alex" && matches!(k, ActivityKind::PrMerged { .. })));
    }

    #[test]
    fn a_rule_that_is_off_still_logs_the_event_but_leaves_the_status() {
        let t = tracker();
        let mut rules = RuleSet::default();
        rules.set(Rule::MergeMovesToDone, false);
        let task = t.create(&NewTask::titled("Manual"), "alex").unwrap();
        t.update(&task.id, &crate::Patch::status(Status::InReview), "alex").unwrap();
        t.record(&task.id, &Entry::PrOpened(pr(7)), "Claude").unwrap();
        let out = handle(&t, &rules, &merged()).unwrap();
        assert_eq!((out[0].task.status, out[0].moved), (Status::InReview, None));
        assert!(kinds(&t, &task.id).iter().any(|(_, k)| matches!(k, ActivityKind::PrMerged { .. })), "the merge is in the log");
    }

    #[test]
    fn a_failed_session_leaves_the_task_in_progress() {
        let t = tracker();
        let rules = RuleSet::default();
        let task = t.create(&NewTask::titled("Hard"), "alex").unwrap();
        handle(&t, &rules, &Signal::SessionStarted { task: task.id.clone(), session: session("s1") }).unwrap();
        let out = handle(&t, &rules, &finished(false)).unwrap();
        assert_eq!((out[0].task.status, out[0].moved), (Status::InProgress, None));
    }

    #[test]
    fn a_signal_for_a_session_or_pull_request_no_task_has_does_nothing() {
        let t = tracker();
        let rules = RuleSet::default();
        t.create(&NewTask::titled("Alone"), "alex").unwrap();
        assert!(handle(&t, &rules, &finished(true)).unwrap().is_empty());
        assert!(handle(&t, &rules, &merged()).unwrap().is_empty());
    }

    #[test]
    fn a_merge_reaches_every_task_the_pull_request_is_linked_to_and_spares_a_closed_one() {
        let t = tracker();
        let rules = RuleSet::default();
        let a = t.create(&NewTask::titled("A"), "x").unwrap();
        let b = t.create(&NewTask::titled("B"), "x").unwrap();
        let c = t.create(&NewTask::titled("C"), "x").unwrap();
        for task in [&a, &b, &c] {
            t.record(&task.id, &Entry::PrOpened(pr(7)), "x").unwrap();
        }
        t.update(&c.id, &crate::Patch::status(Status::Canceled), "x").unwrap();
        let out = handle(&t, &rules, &merged()).unwrap();
        assert_eq!(out.len(), 3);
        let status = |id: &TaskId| t.get(id).unwrap().unwrap().status;
        assert_eq!((status(&a.id), status(&b.id), status(&c.id)), (Status::Done, Status::Done, Status::Canceled));
    }

    #[test]
    fn a_session_finishing_moves_only_the_tasks_it_worked_on() {
        let t = tracker();
        let rules = RuleSet::default();
        let a = t.create(&NewTask::titled("A"), "x").unwrap();
        let b = t.create(&NewTask::titled("B"), "x").unwrap();
        handle(&t, &rules, &Signal::SessionStarted { task: a.id.clone(), session: session("s1") }).unwrap();
        handle(&t, &rules, &Signal::SessionStarted { task: b.id.clone(), session: session("s2") }).unwrap();
        handle(&t, &rules, &finished(true)).unwrap();
        let status = |id: &TaskId| t.get(id).unwrap().unwrap().status;
        assert_eq!((status(&a.id), status(&b.id)), (Status::InReview, Status::InProgress));
    }

    #[test]
    fn a_signal_for_a_task_that_is_gone_is_an_error_not_a_panic() {
        let t = tracker();
        let signal = Signal::SessionStarted { task: TaskId::from("99"), session: session("s1") };
        assert!(handle(&t, &RuleSet::default(), &signal).is_err());
    }
}
#[test]
fn a_reply_in_the_session_moves_a_task_in_review_back_to_in_progress() {
    let rules = RuleSet::default();
    let reply = Signal::SessionResumed { session_id: "s1".into() };
    assert_eq!(to(&rules, Status::InReview, &reply), Some(Status::InProgress));
    for status in [Status::Backlog, Status::Todo, Status::InProgress, Status::Done, Status::Canceled] {
        assert_eq!(to(&rules, status, &reply), None, "{status:?} stays");
    }
    let mut off = RuleSet::default();
    off.set(Rule::SessionResumeMovesToInProgress, false);
    assert_eq!(to(&off, Status::InReview, &reply), None, "the switch turns it off");
    assert_eq!(Rule::from_id("session-resume"), Some(Rule::SessionResumeMovesToInProgress));
    assert_eq!(off.disabled(), ["session-resume"]);
}
