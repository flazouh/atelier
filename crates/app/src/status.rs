//! Where a session stands, from its events, as `docs/agent-panels.md` sets it: Working from a message
//! to the end of its turn, NeedsYou while a question waits, Finished when a turn ends while the reader
//! looks elsewhere, Idle once they look, and Failed when the session ends badly. Pure.

use beui::session_status::{Need, SessionStatus, short_reason};
use lathe_agents::session::{EndReason, Event, TurnOutcome};

/// The status after `event`, from `now`. `seen` says the reader is looking at this session.
pub fn after(now: &SessionStatus, event: &Event, seen: bool) -> SessionStatus {
    match event {
        Event::Permission(_) => SessionStatus::NeedsYou(Need::Approval),
        // An answered or withdrawn question hands the turn back to the agent.
        Event::PermissionCancelled(_) if matches!(now, SessionStatus::NeedsYou(_)) => SessionStatus::Working,
        Event::TurnEnded(end) => match &end.outcome {
            TurnOutcome::Failed(why) => SessionStatus::Failed(short_reason(why)),
            TurnOutcome::Completed | TurnOutcome::Interrupted if seen => SessionStatus::Idle,
            TurnOutcome::Completed | TurnOutcome::Interrupted => SessionStatus::Finished,
        },
        Event::Ended(EndReason::Failed(why)) => SessionStatus::Failed(short_reason(why)),
        Event::Ended(EndReason::Exited(code)) if !matches!(now, SessionStatus::Failed(_)) => match code {
            Some(0) if matches!(now, SessionStatus::Idle | SessionStatus::Finished) => now.clone(),
            Some(code) => SessionStatus::Failed(format!("the agent exited with code {code}").into()),
            None => SessionStatus::Failed("the agent was stopped".into()),
        },
        _ => now.clone(),
    }
}

/// The status once the reader sent a message or answered a question.
pub fn sent() -> SessionStatus {
    SessionStatus::Working
}

/// The status once the reader opened the session: its news is seen.
pub fn opened(now: &SessionStatus) -> SessionStatus {
    match now {
        SessionStatus::Finished => SessionStatus::Idle,
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests;
