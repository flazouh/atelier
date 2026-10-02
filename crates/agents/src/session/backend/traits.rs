use std::sync::Arc;

use atelier_project::Project;

use super::super::{
    command::{Command},
    event::{Event, SessionId},
};
use super::structs::{Capabilities, OpenRequest, SessionSummary};
use super::types::{EventSink, SessionError};

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
