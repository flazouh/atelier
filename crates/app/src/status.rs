//! Where a session stands, from its events, as `docs/agent-panels.md` sets it: Working from a message
//! to the end of its turn, NeedsYou while a question waits, Finished when a turn ends while the reader
//! looks elsewhere, Idle once they look, and Failed when the session ends badly. Pure.

use atelier_ui::session_status::{Need, SessionStatus, short_reason};
use atelier_agents::session::{EndReason, Event, TurnOutcome};

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
        // Stopped by the reader: nothing runs any more, and nothing waits.
        Event::Ended(EndReason::Closed) if matches!(now, SessionStatus::Working | SessionStatus::NeedsYou(_)) => SessionStatus::Idle,
        Event::Ended(EndReason::Exited { code, stderr }) if !matches!(now, SessionStatus::Failed(_)) => match code {
            Some(0) if matches!(now, SessionStatus::Idle | SessionStatus::Finished) => now.clone(),
            // The row says the agent's own last words when it left any.
            _ if !stderr.trim().is_empty() => SessionStatus::Failed(short_reason(last_line(stderr))),
            Some(code) => SessionStatus::Failed(format!("the agent exited with code {code}").into()),
            None => SessionStatus::Failed("the agent was stopped".into()),
        },
        _ => now.clone(),
    }
}

/// The last line of a stream that says anything.
pub fn last_line(text: &str) -> &str {
    text.lines().rfind(|l| !l.trim().is_empty()).unwrap_or("").trim()
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
