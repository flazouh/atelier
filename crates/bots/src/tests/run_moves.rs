use crate::{
    ASKS_FIRST_QUESTION, BotId, BotsError, HandOver, Playbook, PlaybookStep, Run, RunEvent,
    RunState, StepReading, StepState, starter_playbooks,
};

const BRIEF: &str = "Add a dark theme.";

fn id(text: &str) -> BotId {
    text.parse().unwrap()
}

fn deliver() -> Playbook {
    starter_playbooks().remove(0)
}

/// Three bots, and none asks first.
fn trio() -> Playbook {
    let step = |bot: &str| PlaybookStep {
        bot: id(bot),
        asks_first: false,
    };
    Playbook {
        id: id("trio"),
        name: "Trio".into(),
        steps: vec![step("nimbus"), step("bolt"), step("pip")],
    }
}

fn started(playbook: &Playbook) -> Run {
    Run::start(id("run-1"), playbook, BRIEF, 1_000).unwrap().0
}

/// A run of the trio with its first step at work.
fn at_work() -> Run {
    started(&trio()).step_starts(0, "s-0").unwrap().0
}

fn reading(hand_overs: &[(&str, &str)], answer: Option<&str>) -> StepReading {
    StepReading {
        brief: BRIEF.into(),
        hand_overs: hand_overs
            .iter()
            .map(|(bot, text)| HandOver {
                bot: id(bot),
                text: text.to_string(),
            })
            .collect(),
        answer: answer.map(str::to_string),
    }
}

/// The words of a refusal.
fn refusal(result: Result<(Run, Vec<RunEvent>), BotsError>) -> String {
    match result {
        Err(BotsError::Refused(why)) => why,
        other => panic!("not a refusal: {other:?}"),
    }
}

fn states(run: &Run) -> Vec<String> {
    run.steps.iter().map(|s| s.state.to_string()).collect()
}

#[test]
fn a_new_run_has_its_first_step_ready_and_the_rest_wait() {
    let (run, events) = Run::start(id("run-1"), &trio(), "  Add a dark theme. ", 1_000).unwrap();
    assert_eq!(run.id, id("run-1"));
    assert_eq!(run.playbook, id("trio"));
    assert_eq!(run.brief, BRIEF, "the brief is kept without the space around it");
    assert_eq!(run.started_ms, 1_000);
    assert_eq!(states(&run), ["waiting", "waiting", "waiting"]);
    assert_eq!(run.ready_step(), Some(0));
    assert_eq!(run.state(), RunState::Working);
    assert_eq!(
        events,
        [RunEvent::StepReady {
            step: 0,
            bot: id("nimbus"),
            reading: reading(&[], None),
        }]
    );
    assert!(run.problems().is_empty());
}

#[test]
fn a_playbook_with_no_steps_or_an_empty_brief_starts_no_run() {
    let mut empty = trio();
    empty.steps.clear();
    let err = Run::start(id("run-1"), &empty, BRIEF, 1).unwrap_err();
    assert!(
        matches!(&err, BotsError::Invalid(p) if p[0].contains("at least one step")),
        "{err}"
    );
    let err = Run::start(id("run-1"), &trio(), "   ", 1).unwrap_err();
    assert!(
        matches!(&err, BotsError::Invalid(p) if p[0].contains("the brief")),
        "{err}"
    );
}

#[test]
fn a_first_step_that_asks_first_waits_for_a_person_at_once() {
    let mut playbook = trio();
    playbook.steps[0].asks_first = true;
    let (run, events) = Run::start(id("run-1"), &playbook, BRIEF, 1).unwrap();
    assert_eq!(run.state(), RunState::NeedsPerson);
    assert_eq!(run.ready_step(), None);
    assert_eq!(
        events,
        [RunEvent::StepNeedsPerson {
            step: 0,
            bot: id("nimbus"),
            question: ASKS_FIRST_QUESTION.into(),
        }]
    );
}

#[test]
fn a_step_that_starts_is_at_work_with_the_session_the_app_gave() {
    let (run, events) = started(&trio()).step_starts(0, "s-0").unwrap();
    assert_eq!(run.steps[0].state, StepState::Working);
    assert_eq!(run.steps[0].session.as_deref(), Some("s-0"));
    assert_eq!(run.ready_step(), None, "a step at work is not ready again");
    assert_eq!(run.state(), RunState::Working);
    assert_eq!(
        events,
        [RunEvent::StepStarted {
            step: 0,
            bot: id("nimbus"),
            session: "s-0".into(),
        }]
    );
}

#[test]
fn a_move_leaves_the_run_it_is_called_on_as_it_was() {
    let before = started(&trio());
    let copy = before.clone();
    before.step_starts(0, "s-0").unwrap();
    before.skip_step(1).unwrap();
    before.stop().unwrap();
    assert_eq!(before, copy);
}

#[test]
fn a_step_asks_a_person_and_the_answer_goes_to_its_session() {
    let (run, events) = at_work().step_asks(0, "Light too, or dark only?").unwrap();
    assert_eq!(run.state(), RunState::NeedsPerson);
    assert_eq!(
        events,
        [RunEvent::StepNeedsPerson {
            step: 0,
            bot: id("nimbus"),
            question: "Light too, or dark only?".into(),
        }]
    );
    let (run, events) = run.person_answers(0, "Dark only.").unwrap();
    assert_eq!(run.steps[0].state, StepState::Working);
    assert_eq!(run.steps[0].answer.as_deref(), Some("Dark only."));
    assert_eq!(run.state(), RunState::Working);
    assert_eq!(
        events,
        [RunEvent::StepAnswered {
            step: 0,
            bot: id("nimbus"),
            session: "s-0".into(),
            answer: "Dark only.".into(),
        }]
    );
}

#[test]
fn a_step_that_ends_hands_over_and_the_next_step_reads_the_brief_and_the_hand_overs_so_far() {
    let (run, events) = at_work().step_ends(0, " The plan has three parts. ").unwrap();
    assert_eq!(run.steps[0].state, StepState::Done);
    assert_eq!(
        run.steps[0].hand_over.as_deref(),
        Some("The plan has three parts.")
    );
    assert_eq!(run.ready_step(), Some(1));
    assert_eq!(
        events,
        [
            RunEvent::StepDone {
                step: 0,
                bot: id("nimbus"),
                hand_over: Some("The plan has three parts.".into()),
            },
            RunEvent::StepReady {
                step: 1,
                bot: id("bolt"),
                reading: reading(&[("nimbus", "The plan has three parts.")], None),
            },
        ]
    );
    let run = run.step_starts(1, "s-1").unwrap().0;
    let (run, events) = run.step_ends(1, "Built, with tests.").unwrap();
    assert_eq!(
        events[1],
        RunEvent::StepReady {
            step: 2,
            bot: id("pip"),
            reading: reading(
                &[
                    ("nimbus", "The plan has three parts."),
                    ("bolt", "Built, with tests.")
                ],
                None
            ),
        }
    );
    assert_eq!(run.reading(2), Some(reading(
        &[
            ("nimbus", "The plan has three parts."),
            ("bolt", "Built, with tests.")
        ],
        None
    )));
    assert_eq!(run.reading(3), None, "the run has no fourth step");
}

#[test]
fn a_step_that_ends_with_no_words_hands_over_nothing() {
    let (run, events) = at_work().step_ends(0, "  ").unwrap();
    assert_eq!(run.steps[0].hand_over, None);
    assert_eq!(
        events[0],
        RunEvent::StepDone {
            step: 0,
            bot: id("nimbus"),
            hand_over: None,
        }
    );
    assert_eq!(run.reading(1), Some(reading(&[], None)));
}

#[test]
fn a_step_that_asks_first_waits_for_a_person_when_its_turn_comes_and_is_ready_after_the_answer() {
    let mut playbook = trio();
    playbook.steps[1].asks_first = true;
    let run = started(&playbook).step_starts(0, "s-0").unwrap().0;
    let (run, events) = run.step_ends(0, "Planned.").unwrap();
    assert_eq!(run.state(), RunState::NeedsPerson);
    assert_eq!(run.ready_step(), None, "it cannot start before a person says so");
    assert_eq!(
        events[1],
        RunEvent::StepNeedsPerson {
            step: 1,
            bot: id("bolt"),
            question: ASKS_FIRST_QUESTION.into(),
        }
    );
    assert!(refusal(run.step_starts(1, "s-1")).contains("waiting for a person"));
    let (run, events) = run.person_answers(1, "Yes, on a branch.").unwrap();
    assert_eq!(run.ready_step(), Some(1));
    assert_eq!(
        events,
        [RunEvent::StepReady {
            step: 1,
            bot: id("bolt"),
            reading: reading(&[("nimbus", "Planned.")], Some("Yes, on a branch.")),
        }]
    );
    assert!(
        refusal(run.person_answers(1, "And more.")).contains("asked nothing"),
        "a step that is ready waits for no answer"
    );
    let run = run.step_starts(1, "s-1").unwrap().0;
    assert_eq!(run.steps[1].state, StepState::Working);
}

#[test]
fn a_yes_with_no_words_lets_a_step_that_asks_first_start() {
    let mut playbook = trio();
    playbook.steps[0].asks_first = true;
    let (run, events) = started(&playbook).person_answers(0, " ").unwrap();
    assert_eq!(run.steps[0].answer, None);
    assert_eq!(
        events,
        [RunEvent::StepReady {
            step: 0,
            bot: id("nimbus"),
            reading: reading(&[], None),
        }]
    );
}

#[test]
fn the_last_step_that_ends_ends_the_run() {
    let mut run = started(&trio());
    for step in 0..2 {
        run = run.step_starts(step, "s").unwrap().0;
        run = run.step_ends(step, "Done.").unwrap().0;
    }
    run = run.step_starts(2, "s-2").unwrap().0;
    let (run, events) = run.step_ends(2, "It works.").unwrap();
    assert_eq!(run.state(), RunState::Done);
    assert_eq!(run.current_step(), None);
    assert_eq!(
        events,
        [
            RunEvent::StepDone {
                step: 2,
                bot: id("pip"),
                hand_over: Some("It works.".into()),
            },
            RunEvent::RunDone,
        ]
    );
}

#[test]
fn a_step_that_fails_fails_the_run_with_its_reason() {
    let (run, events) = at_work().step_fails(0, "The agent ran out of quota.").unwrap();
    assert_eq!(
        run.steps[0].state,
        StepState::Failed {
            reason: "The agent ran out of quota.".into()
        }
    );
    assert_eq!(run.state(), RunState::Failed);
    assert_eq!(run.ready_step(), None);
    assert_eq!(
        events,
        [RunEvent::RunFailed {
            step: 0,
            bot: id("nimbus"),
            reason: "The agent ran out of quota.".into(),
        }]
    );
}

#[test]
fn a_ready_step_can_fail_before_it_starts_and_a_step_that_waits_for_a_person_can_fail() {
    let (run, _) = started(&trio()).step_fails(0, "The program is not installed.").unwrap();
    assert_eq!(run.state(), RunState::Failed);
    assert_eq!(run.steps[0].session, None);
    let asked = at_work().step_asks(0, "Which one?").unwrap().0;
    assert_eq!(
        asked.step_fails(0, "The session closed.").unwrap().0.state(),
        RunState::Failed
    );
}

#[test]
fn a_failed_step_can_be_tried_again_with_a_new_session() {
    let failed = at_work().step_fails(0, "Out of quota.").unwrap().0;
    let (run, events) = failed.retry_step(0).unwrap();
    assert_eq!(run.steps[0].state, StepState::Waiting);
    assert_eq!(run.steps[0].session, None, "the old session is not used again");
    assert_eq!(run.ready_step(), Some(0));
    assert_eq!(run.state(), RunState::Working);
    assert_eq!(
        events,
        [RunEvent::StepReady {
            step: 0,
            bot: id("nimbus"),
            reading: reading(&[], None),
        }]
    );
    let run = run.step_starts(0, "s-again").unwrap().0;
    assert_eq!(run.steps[0].session.as_deref(), Some("s-again"));
}

#[test]
fn a_person_can_skip_the_step_that_has_its_turn_and_the_next_one_is_ready() {
    let (run, events) = started(&trio()).skip_step(0).unwrap();
    assert_eq!(states(&run), ["skipped", "waiting", "waiting"]);
    assert_eq!(run.ready_step(), Some(1));
    assert_eq!(
        events,
        [
            RunEvent::StepSkipped {
                step: 0,
                bot: id("nimbus"),
                session: None,
            },
            RunEvent::StepReady {
                step: 1,
                bot: id("bolt"),
                reading: reading(&[], None),
            },
        ]
    );
}

#[test]
fn a_person_can_skip_a_later_step_ahead_of_its_turn_and_the_run_passes_over_it() {
    let (run, events) = at_work().skip_step(1).unwrap();
    assert_eq!(states(&run), ["at work", "skipped", "waiting"]);
    assert_eq!(
        events,
        [RunEvent::StepSkipped {
            step: 1,
            bot: id("bolt"),
            session: None,
        }],
        "the step at work goes on, and no other step is ready yet"
    );
    let (run, events) = run.step_ends(0, "Planned.").unwrap();
    assert_eq!(run.ready_step(), Some(2));
    assert_eq!(
        events[1],
        RunEvent::StepReady {
            step: 2,
            bot: id("pip"),
            reading: reading(&[("nimbus", "Planned.")], None),
        }
    );
}

#[test]
fn a_skipped_step_at_work_tells_its_session_so_the_app_can_end_it() {
    let (_, events) = at_work().skip_step(0).unwrap();
    assert_eq!(
        events[0],
        RunEvent::StepSkipped {
            step: 0,
            bot: id("nimbus"),
            session: Some("s-0".into()),
        }
    );
}

#[test]
fn a_person_can_skip_a_failed_step_and_a_run_with_its_last_step_skipped_is_done() {
    let run = at_work().step_fails(0, "Out of quota.").unwrap().0;
    let (run, _) = run.skip_step(0).unwrap();
    assert_eq!(run.state(), RunState::Working);
    assert_eq!(run.ready_step(), Some(1));
    let run = run.skip_step(1).unwrap().0;
    let (run, events) = run.skip_step(2).unwrap();
    assert_eq!(run.state(), RunState::Done);
    assert_eq!(events.last(), Some(&RunEvent::RunDone));
}

#[test]
fn a_person_can_run_one_step_alone() {
    let (run, events) =
        Run::start_alone(id("review-1"), &deliver(), 3, "Review pull request 12.", 5).unwrap();
    assert_eq!(
        states(&run),
        ["skipped", "skipped", "skipped", "waiting", "skipped"]
    );
    assert_eq!(run.playbook, id("deliver"));
    assert_eq!(run.ready_step(), Some(3));
    assert_eq!(
        events,
        [RunEvent::StepReady {
            step: 3,
            bot: id("olive"),
            reading: StepReading {
                brief: "Review pull request 12.".into(),
                hand_overs: vec![],
                answer: None,
            },
        }]
    );
    let run = run.step_starts(3, "s-olive").unwrap().0;
    let (run, events) = run.step_ends(3, "Two findings.").unwrap();
    assert_eq!(run.state(), RunState::Done);
    assert_eq!(events.last(), Some(&RunEvent::RunDone));
}

#[test]
fn a_step_run_alone_does_not_ask_first_and_a_step_the_playbook_lacks_is_refused() {
    let (run, _) = Run::start_alone(id("ship-1"), &deliver(), 4, "Ship it.", 5).unwrap();
    assert_eq!(
        run.ready_step(),
        Some(4),
        "the person who picked the step already said go"
    );
    assert!(
        refusal(Run::start_alone(id("x"), &deliver(), 5, "Ship it.", 5)).contains("no step 6")
    );
}

#[test]
fn a_person_can_stop_a_run_and_the_event_names_the_session_to_end() {
    let (run, events) = at_work().stop().unwrap();
    assert!(run.stopped);
    assert_eq!(run.state(), RunState::Stopped);
    assert_eq!(run.ready_step(), None);
    assert_eq!(states(&run), ["at work", "waiting", "waiting"], "the steps show where it stopped");
    assert_eq!(
        events,
        [RunEvent::RunStopped {
            session: Some("s-0".into())
        }]
    );
    let (_, events) = started(&trio()).stop().unwrap();
    assert_eq!(events, [RunEvent::RunStopped { session: None }]);
}

#[test]
fn a_failed_run_can_be_stopped() {
    let failed = at_work().step_fails(0, "Out of quota.").unwrap().0;
    assert_eq!(failed.stop().unwrap().0.state(), RunState::Stopped);
}

#[test]
fn every_move_on_a_stopped_run_is_refused() {
    let stopped = at_work().stop().unwrap().0;
    let moves = [
        stopped.step_starts(1, "s"),
        stopped.step_asks(0, "?"),
        stopped.person_answers(0, "yes"),
        stopped.step_ends(0, "Done."),
        stopped.step_fails(0, "Why."),
        stopped.skip_step(1),
        stopped.retry_step(0),
        stopped.stop(),
    ];
    for result in moves {
        assert!(refusal(result).contains("the run is stopped"));
    }
}

#[test]
fn a_step_that_never_started_cannot_end() {
    let run = started(&trio());
    assert!(refusal(run.step_ends(0, "Done.")).contains("step 1 never started"));
    assert!(refusal(run.step_ends(2, "Done.")).contains("step 3 never started"));
}

#[test]
fn a_step_cannot_start_out_of_order() {
    let run = started(&trio());
    let why = refusal(run.step_starts(1, "s-1"));
    assert!(
        why.contains("step 2 cannot start") && why.contains("step 1"),
        "{why}"
    );
    let run = at_work();
    let why = refusal(run.step_starts(2, "s-2"));
    assert!(why.contains("step 3 cannot start"), "{why}");
}

#[test]
fn a_step_cannot_start_twice_or_after_it_is_over_or_with_no_session() {
    assert!(refusal(at_work().step_starts(0, "s-b")).contains("step 1 is at work"));
    let done = at_work().step_ends(0, "Done.").unwrap().0;
    assert!(refusal(done.step_starts(0, "s-b")).contains("step 1 is done"));
    let failed = at_work().step_fails(0, "Why.").unwrap().0;
    assert!(refusal(failed.step_starts(0, "s-b")).contains("step 1 is failed"));
    assert!(refusal(started(&trio()).step_starts(0, "  ")).contains("session"));
}

#[test]
fn only_a_step_at_work_can_ask_or_end() {
    let run = started(&trio());
    assert!(refusal(run.step_asks(0, "?")).contains("step 1 is waiting"));
    let asked = at_work().step_asks(0, "Which one?").unwrap().0;
    assert!(refusal(asked.step_asks(0, "And this?")).contains("waiting for a person"));
    assert!(refusal(asked.step_ends(0, "Done.")).contains("waiting for a person"));
    let done = at_work().step_ends(0, "Done.").unwrap().0;
    assert!(refusal(done.step_ends(0, "Again.")).contains("step 1 is done"));
}

#[test]
fn a_person_cannot_answer_a_step_that_asked_nothing() {
    assert!(refusal(at_work().person_answers(0, "Yes.")).contains("asked nothing"));
    assert!(refusal(started(&trio()).person_answers(1, "Yes.")).contains("asked nothing"));
}

#[test]
fn a_step_cannot_fail_with_no_reason_or_before_its_turn_or_when_it_is_over() {
    assert!(refusal(at_work().step_fails(0, " ")).contains("a reason"));
    let why = refusal(at_work().step_fails(1, "Why."));
    assert!(why.contains("step 2 cannot fail"), "{why}");
    let done = at_work().step_ends(0, "Done.").unwrap().0;
    assert!(refusal(done.step_fails(0, "Why.")).contains("step 1 is done"));
    let failed = at_work().step_fails(0, "Why.").unwrap().0;
    assert!(refusal(failed.step_fails(0, "Again.")).contains("step 1 is failed"));
}

#[test]
fn only_a_failed_step_can_be_tried_again() {
    assert!(refusal(at_work().retry_step(0)).contains("only a failed step"));
    let done = at_work().step_ends(0, "Done.").unwrap().0;
    assert!(refusal(done.retry_step(0)).contains("step 1 is done"));
}

#[test]
fn a_step_that_is_over_cannot_be_skipped() {
    let done = at_work().step_ends(0, "Done.").unwrap().0;
    assert!(refusal(done.skip_step(0)).contains("step 1 is done"));
    let skipped = started(&trio()).skip_step(1).unwrap().0;
    assert!(refusal(skipped.skip_step(1)).contains("step 2 is skipped"));
}

#[test]
fn a_run_that_is_done_cannot_be_stopped_and_nothing_else_moves_it() {
    let mut run = started(&trio());
    for step in 0..3 {
        run = run.skip_step(step).unwrap().0;
    }
    assert!(refusal(run.stop()).contains("the run is done"));
    assert!(refusal(run.step_starts(0, "s")).contains("step 1 is skipped"));
    assert!(refusal(run.retry_step(2)).contains("step 3 is skipped"));
}

#[test]
fn a_step_the_run_does_not_have_is_refused() {
    let run = started(&trio());
    for result in [
        run.step_starts(3, "s"),
        run.step_asks(3, "?"),
        run.person_answers(3, "yes"),
        run.step_ends(3, "Done."),
        run.step_fails(3, "Why."),
        run.skip_step(3),
        run.retry_step(3),
    ] {
        assert!(refusal(result).contains("there is no step 4"));
    }
}

#[test]
fn a_run_that_breaks_a_rule_is_not_moved() {
    let mut run = started(&trio());
    run.steps[2].state = StepState::Working;
    let err = run.step_starts(0, "s-0").unwrap_err();
    assert!(matches!(&err, BotsError::Invalid(p) if p[0].contains("step 3")), "{err}");
}

#[test]
fn a_whole_deliver_run_goes_from_the_brief_to_done() {
    let (mut run, events) = Run::start(id("run-1"), &deliver(), BRIEF, 1_000).unwrap();
    assert_eq!(
        events,
        [RunEvent::StepReady {
            step: 0,
            bot: id("nimbus"),
            reading: reading(&[], None),
        }]
    );
    let crew = ["nimbus", "bolt", "pip", "olive", "skip"];
    let mut said: Vec<(&str, String)> = Vec::new();
    for (step, bot) in crew.iter().enumerate() {
        if step == 4 {
            // The Shipper asks first.
            assert_eq!(run.state(), RunState::NeedsPerson);
            run = run.person_answers(step, "Ship it.").unwrap().0;
        }
        assert_eq!(run.ready_step(), Some(step));
        let read = run.reading(step).unwrap();
        assert_eq!(read.brief, BRIEF);
        assert_eq!(
            read.hand_overs
                .iter()
                .map(|h| (h.bot.as_str(), h.text.clone()))
                .collect::<Vec<_>>(),
            said,
            "{bot} reads every hand-over before it, in order"
        );
        run = run.step_starts(step, &format!("s-{bot}")).unwrap().0;
        assert_eq!(run.state(), RunState::Working);
        let text = format!("{bot} is done.");
        let (next, events) = run.step_ends(step, &text).unwrap();
        run = next;
        said.push((*bot, text));
        let after = match step {
            3 => RunEvent::StepNeedsPerson {
                step: 4,
                bot: id("skip"),
                question: ASKS_FIRST_QUESTION.into(),
            },
            4 => RunEvent::RunDone,
            _ => RunEvent::StepReady {
                step: step + 1,
                bot: id(crew[step + 1]),
                reading: run.reading(step + 1).unwrap(),
            },
        };
        assert_eq!(events.len(), 2);
        assert_eq!(events[1], after);
    }
    assert_eq!(run.state(), RunState::Done);
    assert_eq!(states(&run), ["done"; 5]);
    assert_eq!(run.steps[4].answer.as_deref(), Some("Ship it."));
    assert!(run.problems().is_empty());
}

#[test]
fn a_run_that_fails_and_is_tried_again_goes_on_to_done() {
    let run = started(&trio()).step_starts(0, "s-0").unwrap().0;
    let run = run.step_ends(0, "Planned.").unwrap().0;
    let run = run.step_starts(1, "s-1").unwrap().0;
    let (run, events) = run.step_fails(1, "The build broke.").unwrap();
    assert_eq!(run.state(), RunState::Failed);
    assert!(matches!(&events[0], RunEvent::RunFailed { step: 1, reason, .. } if reason == "The build broke."));
    assert!(refusal(run.step_starts(2, "s-2")).contains("step 3 cannot start"));
    let (run, events) = run.retry_step(1).unwrap();
    assert_eq!(
        events,
        [RunEvent::StepReady {
            step: 1,
            bot: id("bolt"),
            reading: reading(&[("nimbus", "Planned.")], None),
        }],
        "the step reads the same brief and hand-overs again"
    );
    let run = run.step_starts(1, "s-1b").unwrap().0;
    let run = run.step_ends(1, "Built.").unwrap().0;
    let run = run.step_starts(2, "s-2").unwrap().0;
    let (run, events) = run.step_ends(2, "Checked.").unwrap();
    assert_eq!(run.state(), RunState::Done);
    assert_eq!(events.last(), Some(&RunEvent::RunDone));
    assert_eq!(run.steps[1].session.as_deref(), Some("s-1b"));
    assert!(run.problems().is_empty());
}
