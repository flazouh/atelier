use atelier_bot_face::Mood;
use atelier_ui::session_status::{Need, SessionStatus};

use super::super::mood_of;

#[test]
fn a_session_that_waits_is_idle() {
    assert_eq!(mood_of(&SessionStatus::Idle, false), Mood::Idle);
}

#[test]
fn a_session_whose_agent_plans_is_thinking() {
    assert_eq!(mood_of(&SessionStatus::Working, false), Mood::Thinking);
}

#[test]
fn a_session_that_runs_a_tool_is_working() {
    assert_eq!(mood_of(&SessionStatus::Working, true), Mood::Working);
}

#[test]
fn a_session_that_finished_and_was_not_seen_is_done() {
    assert_eq!(mood_of(&SessionStatus::Finished, false), Mood::Done);
}

#[test]
fn a_session_that_waits_for_an_approval_or_an_answer_needs_you() {
    assert_eq!(mood_of(&SessionStatus::NeedsYou(Need::Approval), false), Mood::Needs);
    assert_eq!(mood_of(&SessionStatus::NeedsYou(Need::Question), true), Mood::Needs, "the tool that waits does not make it working");
}

#[test]
fn a_session_that_failed_is_stuck() {
    assert_eq!(mood_of(&SessionStatus::Failed("the agent exited with code 3".into()), false), Mood::Stuck);
}

/// Only work shows a tool: a call left open by a turn that ended changes no other mood.
#[test]
fn a_tool_left_open_changes_no_mood_but_the_working_one() {
    for (status, mood) in [
        (SessionStatus::Idle, Mood::Idle),
        (SessionStatus::Finished, Mood::Done),
        (SessionStatus::Failed("x".into()), Mood::Stuck),
    ] {
        assert_eq!(mood_of(&status, true), mood, "{status:?}");
    }
}
