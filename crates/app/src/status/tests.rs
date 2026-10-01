use atelier_agents::session::{PermissionRequest, RequestId, ToolCall, ToolId, ToolKind, ToolStatus, TurnEnd};

use super::*;

fn turn(outcome: TurnOutcome) -> Event {
    Event::TurnEnded(TurnEnd { outcome, summary: None })
}

fn ask() -> Event {
    Event::Permission(PermissionRequest {
        id: RequestId("r1".into()),
        call: ToolCall {
            id: ToolId("t1".into()),
            name: "Edit".into(),
            kind: ToolKind::Edit,
            input: serde_json::json!({}),
            file: None,
            parent: None,
            status: ToolStatus::Pending,
        },
        reason: None,
        choices: Vec::new(),
    })
}

#[test]
fn a_turn_works_asks_and_ends_finished_unless_the_reader_looks() {
    let s = sent();
    assert_eq!(s, SessionStatus::Working);
    let s = after(&s, &ask(), false);
    assert_eq!(s, SessionStatus::NeedsYou(Need::Approval));
    let s = sent();
    let unseen = after(&s, &turn(TurnOutcome::Completed), false);
    assert_eq!(unseen, SessionStatus::Finished);
    assert_eq!(opened(&unseen), SessionStatus::Idle, "opening clears the amber");
    assert_eq!(after(&s, &turn(TurnOutcome::Completed), true), SessionStatus::Idle, "a turn the reader watched is seen");
    assert_eq!(after(&s, &turn(TurnOutcome::Interrupted), false), SessionStatus::Finished);
}

#[test]
fn a_failure_says_why_short() {
    let failed = after(&SessionStatus::Working, &turn(TurnOutcome::Failed("the agent exited with code 1\nmore".into())), true);
    assert_eq!(failed, SessionStatus::Failed("the agent exited with code 1".into()));
    let exited = |code, stderr: &str| Event::Ended(EndReason::Exited { code, stderr: stderr.into() });
    let crashed = after(&SessionStatus::Working, &exited(Some(137), ""), true);
    assert_eq!(crashed, SessionStatus::Failed("the agent exited with code 137".into()));
    let said = after(&SessionStatus::Working, &exited(Some(1), "starting\nError: not logged in. Run claude login.\n\n"), true);
    assert_eq!(said, SessionStatus::Failed("Error: not logged in. Run claude login.".into()), "its own last words");
    assert_eq!(after(&failed, &exited(Some(1), ""), true), failed, "the first reason stays");
    assert_eq!(after(&SessionStatus::Idle, &exited(Some(0), ""), true), SessionStatus::Idle);
    assert_eq!(opened(&failed), failed, "opening a failed session keeps its reason");
}

#[test]
fn a_session_stopped_mid_turn_is_no_longer_working() {
    assert_eq!(after(&SessionStatus::Working, &Event::Ended(EndReason::Closed), true), SessionStatus::Idle);
    assert_eq!(after(&SessionStatus::NeedsYou(Need::Approval), &Event::Ended(EndReason::Closed), false), SessionStatus::Idle);
    assert_eq!(after(&SessionStatus::Finished, &Event::Ended(EndReason::Closed), false), SessionStatus::Finished, "its news stays");
}
