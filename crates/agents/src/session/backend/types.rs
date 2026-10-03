use std::{fmt, sync::Arc};

use super::super::event::Event;

/// Where a session sends its events. It runs on the session's own thread, so it must be quick: queue
/// the event and wake the UI (see `super::EventQueue`).
pub type EventSink = Arc<dyn Fn(Event) + Send + Sync>;

/// Who serves a session's model and is billed for it. An agent with no choice of provider ignores it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Provider {
    /// One of the agent's own sign-ins, each in its own folder. `default` is the agent's usual one.
    Account(String),
    /// OpenRouter's API, paid per token with this key.
    OpenRouter { key: ApiKey },
}

/// A secret that never shows in a log or a debug print.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(hidden)")
    }
}

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
