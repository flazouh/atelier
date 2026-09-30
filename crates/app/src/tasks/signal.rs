//! What a session tells its tasks. The session says three things (it started, a turn ended, the reader
//! replied); this turns each into the tracker's `Signal`, which the rules read. Pure.
use lathe_tracker::{PrLink, SessionLink, Signal};

/// Who made a commit or opened a pull request from the review: the person at the keys.
const READER: &str = "you";

use super::TaskRef;

/// What happened in a session, as the session sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskEvent {
    /// The agent started, for a session begun from a task.
    Started,
    /// A turn ended. `ok` is false when it failed or the reader interrupted it.
    TurnEnded { ok: bool },
    /// The agent started for a session that has no task yet: it may be linked to one already (a session
    /// the app opened again after a restart).
    Adopt,
    /// The reader sent a message after the first.
    Replied,
    /// The reader made a commit from the session's review.
    Committed { sha: String, subject: String },
    /// The forge opened a pull request for the session's branch.
    PrOpened { number: u64, repo: String },
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
        TaskEvent::Adopt => None,
        TaskEvent::Started => Some(Signal::SessionStarted {
            task: task?.id.clone(),
            session: SessionLink { session_id: session.id.into(), title: session.title.into(), agent: session.agent.into() },
        }),
        TaskEvent::TurnEnded { ok } => Some(Signal::SessionFinished { session_id: session.id.into(), ok }),
        TaskEvent::Replied => Some(Signal::SessionResumed { session_id: session.id.into() }),
        TaskEvent::Committed { sha, subject } => {
            Some(Signal::Committed { session_id: session.id.into(), sha, subject, by: READER.into() })
        }
        TaskEvent::PrOpened { number, repo } => {
            Some(Signal::PrOpened { task: task?.id.clone(), pr: PrLink { number, repo }, by: READER.into() })
        }
    }
}
