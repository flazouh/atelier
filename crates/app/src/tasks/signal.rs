//! What a session tells its tasks. The session says three things (it started, a turn ended, the reader
//! replied); this turns each into the tracker's `Signal`, which the rules read. Pure.
use lathe_tracker::{Signal, SessionLink};

use super::TaskRef;

/// What happened in a session, as the session sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskEvent {
    /// The agent started, for a session begun from a task.
    Started,
    /// A turn ended. `ok` is false when it failed or the reader interrupted it.
    TurnEnded { ok: bool },
    /// The reader sent a message after the first.
    Replied,
}

/// The session, as the tracker links it.
pub struct SessionRef<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub agent: &'a str,
}

/// The signal for `event`. `Started` needs the task the session began from; without one it says nothing.
pub fn of(event: TaskEvent, task: Option<&TaskRef>, session: &SessionRef) -> Option<Signal> {
    match event {
        TaskEvent::Started => Some(Signal::SessionStarted {
            task: task?.id.clone(),
            session: SessionLink { session_id: session.id.into(), title: session.title.into(), agent: session.agent.into() },
        }),
        TaskEvent::TurnEnded { ok } => Some(Signal::SessionFinished { session_id: session.id.into(), ok }),
        TaskEvent::Replied => Some(Signal::SessionResumed { session_id: session.id.into() }),
    }
}
