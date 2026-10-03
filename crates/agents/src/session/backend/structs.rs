use super::super::{command::PermissionMode, event::SessionId};
use super::types::Provider;

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
    /// The agent as it is set up on the host when `None`.
    pub provider: Option<Provider>,
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
