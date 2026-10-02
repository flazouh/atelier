use std::{fmt, sync::Arc};

use super::super::event::Event;

/// Where a session sends its events. It runs on the session's own thread, so it must be quick: queue
/// the event and wake the UI (see [`super::EventQueue`]).
pub type EventSink = Arc<dyn Fn(Event) + Send + Sync>;

#[derive(Debug)]
pub enum SessionError {
    /// The agent's program is not on the host.
    Missing { program: String },
    Start(String),
    /// The session is over.
    Closed,
    Unsupported(&'static str),
    Read(String),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { program } => write!(f, "{program} is not installed on this host"),
            Self::Start(why) => write!(f, "the agent did not start: {why}"),
            Self::Closed => f.write_str("the session is closed"),
            Self::Unsupported(what) => write!(f, "this agent does not support {what}"),
            Self::Read(why) => write!(f, "could not read the agent's records: {why}"),
        }
    }
}

impl std::error::Error for SessionError {}
