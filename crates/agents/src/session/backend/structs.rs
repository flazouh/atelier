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
    /// It runs on a [`super::types::Provider`] the session picks.
    pub providers: bool,
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

/// One of the agent's sign-ins on a host, which a [`super::types::Provider::Account`] names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    /// The agent's usual sign-in is `default`.
    pub name: String,
    pub signed_in: bool,
    /// The subscription, as the agent names it, such as `max`.
    pub plan: Option<String>,
    pub email: Option<String>,
}

/// One row of a project's session list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSummary {
    pub id: SessionId,
    /// The first thing the user said, cut short.
    pub title: String,
    /// Seconds since the Unix epoch of the last activity, when known.
    pub updated: Option<u64>,
    /// The agent's sign-in the session was saved under; its usual one when `None`.
    pub account: Option<String>,
}
