//! The seam between atelier and an agent. A backend starts sessions; a session takes commands and
//! pushes events into a sink. Neither says how: a session may hold a process, a socket or a loop that
//! runs in this process.
use std::{fmt, sync::Arc};

use atelier_project::Project;

use super::{
    command::{Command, PermissionMode},
    event::{Event, SessionId},
};

/// Where a session sends its events. It runs on the session's own thread, so it must be quick: queue
/// the event and wake the UI (see [`super::EventQueue`]).
pub type EventSink = Arc<dyn Fn(Event) + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelChoice {
    pub id: String,
    pub label: String,
}

/// What a backend can do. The UI hides what a backend lacks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub resume: bool,
    pub interrupt: bool,
    /// The models a session can switch to; empty when it cannot switch.
    pub models: Vec<ModelChoice>,
    /// The modes a session can switch to; empty when it cannot switch.
    pub permission_modes: Vec<PermissionMode>,
    pub thinking: bool,
    pub subagents: bool,
    pub todos: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenRequest {
    /// A session to continue; a new one when `None`.
    pub resume: Option<SessionId>,
    pub model: Option<String>,
    pub mode: Option<PermissionMode>,
}

/// One row of a project's session list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSummary {
    pub id: SessionId,
    /// The first thing the user said, cut short.
    pub title: String,
    /// Seconds since the Unix epoch of the last activity, when known.
    pub updated: Option<u64>,
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

pub trait Backend: Send + Sync {
    /// A stable name for settings and logs, such as `claude-code`.
    fn name(&self) -> &str;

    fn capabilities(&self) -> Capabilities;

    /// Starts or resumes a session in `project`. It does not block on the agent: the `Started` event
    /// says when the agent is ready. Dropping the session stops the agent.
    fn open(
        &self,
        project: Arc<dyn Project>,
        request: OpenRequest,
        sink: EventSink,
    ) -> Result<Box<dyn Session>, SessionError>;

    /// The project's past sessions, newest first. May be slow: never call it on the UI thread.
    fn sessions(&self, _project: &dyn Project) -> Result<Vec<SessionSummary>, SessionError> {
        Err(SessionError::Unsupported("a session list"))
    }

    /// What a past session said, as events, for a UI that shows a resumed session. May be slow.
    fn history(&self, _project: &dyn Project, _session: &SessionId) -> Result<Vec<Event>, SessionError> {
        Err(SessionError::Unsupported("history"))
    }

    /// One-shot text for `prompt`, on `model` or the agent's default, with no session kept anywhere and
    /// nothing changed: a commit message, a branch name, a pull request's title. Blocks, and may take
    /// seconds: never call it on the UI thread.
    fn draft(&self, _project: &dyn Project, _prompt: &str, _model: Option<&str>) -> Result<String, SessionError> {
        Err(SessionError::Unsupported("drafts"))
    }
}

pub trait Session: Send {
    /// Queues a command. It never waits for the agent.
    fn send(&self, command: Command) -> Result<(), SessionError>;
}
