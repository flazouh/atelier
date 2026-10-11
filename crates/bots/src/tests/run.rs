use crate::{BotId, Playbook, PlaybookStep, Run, RunState, RunStep, StepState, starter_playbooks};

fn id(text: &str) -> BotId {
    text.parse().unwrap()
}

fn step(bot: &str, state: StepState) -> RunStep {
    RunStep {
        bot: id(bot),
        asks_first: false,
        state,
        session: None,
        hand_over: None,
        answer: None,
    }
}

fn run(steps: Vec<RunStep>) -> Run {
    Run {
        id: id("run-1"),
        playbook: id("deliver"),
        brief: "Add a dark theme.".into(),
        started_ms: 1_000,
        stopped: false,
        steps,
    }
}

fn started(session: &str, state: StepState) -> RunStep {
    RunStep {
        session: Some(session.into()),
        ..step("bolt", state)
    }
}

fn failed() -> StepState {
    StepState::Failed {
        reason: "Out of quota.".into(),
    }
}

fn asks() -> StepState {
    StepState::NeedsPerson {
        question: "Which one?".into(),
    }
}

#[test]
fn the_state_of_a_run_is_read_from_the_step_that_has_its_turn() {
    let done = || RunStep {
        hand_over: Some("Done.".into()),
        ..started("s-0", StepState::Done)
    };
    let waits = || step("pip", StepState::Waiting);
    let cases = [
        (vec![waits(), waits()], RunState::Working),
        (vec![done(), started("s-1", StepState::Working)], RunState::Working),
        (vec![done(), started("s-1", asks()), waits()], RunState::NeedsPerson),
        (vec![done(), step("bolt", asks())], RunState::NeedsPerson),
        (vec![done(), started("s-1", failed()), waits()], RunState::Failed),
        (vec![done(), step("pip", StepState::Skipped)], RunState::Done),
    ];
    for (steps, state) in cases {
        let mut run = run(steps);
        assert!(run.problems().is_empty(), "{:?}", run.problems());
        assert_eq!(run.state(), state);
        run.stopped = true;
        assert_eq!(run.state(), RunState::Stopped, "a stop wins over every step");
    }
}

#[test]
fn a_run_from_a_starter_playbook_has_no_problem() {
    for playbook in starter_playbooks() {
        let (run, _) = Run::start(id("run-1"), &playbook, "Add a dark theme.", 1).unwrap();
        assert!(run.problems().is_empty(), "{:?}", run.problems());
        assert_eq!(run.steps.len(), playbook.steps.len());
    }
}

#[test]
fn a_run_copies_the_bots_and_the_asks_first_marks_of_its_playbook() {
    let playbook = Playbook {
        id: id("pair"),
        name: "Pair".into(),
        steps: vec![
            PlaybookStep {
                bot: id("bolt"),
                asks_first: false,
            },
            PlaybookStep {
                bot: id("skip"),
                asks_first: true,
            },
        ],
    };
    let (run, _) = Run::start(id("run-1"), &playbook, "Ship the fix.", 1).unwrap();
    assert_eq!(
        run.steps
            .iter()
            .map(|s| (s.bot.as_str(), s.asks_first))
            .collect::<Vec<_>>(),
        [("bolt", false), ("skip", true)]
    );
}

#[test]
fn every_broken_rule_of_a_run_is_listed_together() {
    let mut bad = run(vec![]);
    bad.brief = "  ".into();
    let problems = bad.problems();
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(problems.iter().any(|p| p.contains("run-1: the brief")));
    assert!(problems.iter().any(|p| p.contains("at least one step")));
    let mut long = run(vec![step("bolt", StepState::Waiting)]);
    long.brief = "x".repeat(20_001);
    assert!(long.problems()[0].contains("the brief"));
}

#[test]
fn a_step_cannot_be_ahead_of_a_step_that_is_not_over() {
    let bad = run(vec![
        step("nimbus", StepState::Waiting),
        started("s-1", StepState::Working),
        step("pip", StepState::Skipped),
    ]);
    let problems = bad.problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("step 2 is at work") && problems[0].contains("step 1"),
        "{problems:?}"
    );
}

#[test]
fn the_parts_of_a_step_must_fit_its_state() {
    let cases = [
        (started("s-0", StepState::Waiting), "waits and has a session"),
        (step("bolt", StepState::Working), "has no session"),
        (step("bolt", StepState::Done), "has no session"),
        (
            RunStep {
                hand_over: Some("Half.".into()),
                ..started("s-0", StepState::Working)
            },
            "a hand-over",
        ),
        (step("bolt", StepState::Failed { reason: " ".into() }), "no reason"),
    ];
    for (bad, words) in cases {
        let problems = run(vec![bad]).problems();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains("step 1") && problems[0].contains(words),
            "{problems:?}"
        );
    }
}

#[test]
fn a_run_needs_the_bots_of_its_steps_that_are_not_over_and_none_once_it_is_stopped() {
    let done = RunStep {
        session: Some("s-0".into()),
        hand_over: Some("Planned.".into()),
        ..step("nimbus", StepState::Done)
    };
    let mut run = run(vec![
        done,
        step("olive", StepState::Skipped),
        started("s-2", failed()),
        step("pip", StepState::Waiting),
        step("bolt", StepState::Waiting),
    ]);
    assert_eq!(run.bots_needed(), [id("bolt"), id("pip")], "each bot once");
    run.stopped = true;
    assert!(run.bots_needed().is_empty());
}

#[test]
fn the_run_on_disk_has_the_shape_the_docs_promise() {
    let run = run(vec![
        RunStep {
            hand_over: Some("Planned.".into()),
            ..started("s-0", StepState::Done)
        },
        RunStep {
            asks_first: true,
            answer: Some("Go.".into()),
            ..started("s-1", failed())
        },
        step("pip", StepState::Waiting),
    ]);
    let json = serde_json::to_value(&run).unwrap();
    assert_eq!(json["id"], "run-1");
    assert_eq!(json["playbook"], "deliver");
    assert_eq!(json["brief"], "Add a dark theme.");
    assert_eq!(json["started_ms"], 1_000);
    assert_eq!(json["stopped"], false);
    assert_eq!(json.get("state"), None, "the state of a run is not kept");
    assert_eq!(
        json["steps"][0],
        serde_json::json!({
            "bot": "bolt", "asks_first": false, "state": "done",
            "session": "s-0", "hand_over": "Planned.", "answer": null
        })
    );
    assert_eq!(
        json["steps"][1]["state"],
        serde_json::json!({ "failed": { "reason": "Out of quota." } })
    );
    assert_eq!(json["steps"][2]["state"], "waiting");
    let back: Run = serde_json::from_value(json).unwrap();
    assert_eq!(back, run);
    let short: Run = serde_json::from_str(
        r#"{ "id": "r", "playbook": "deliver", "brief": "B", "started_ms": 1,
             "steps": [{ "bot": "bolt", "state": { "needs-person": { "question": "Which?" } } }] }"#,
    )
    .unwrap();
    assert!(!short.stopped);
    assert_eq!(short.state(), RunState::NeedsPerson);
    let escapes = r#"{ "id": "../x", "playbook": "deliver", "brief": "B", "started_ms": 1, "steps": [] }"#;
    assert!(
        serde_json::from_str::<Run>(escapes).is_err(),
        "an id that could leave the folder is refused when it is read"
    );
}
