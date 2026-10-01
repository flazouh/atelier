use atelier_tracker::{PrLink, SessionLink, Signal};

use super::super::TaskRef;
use super::structs::SessionRef;
use super::types::{READER, TaskEvent};

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
