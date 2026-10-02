use std::{
    time::{Duration},
};

use crate::{
    session::{Event, OpenRequest, SessionId, SessionSummary},
};

/// The JSON-RPC error code ACP gives when the agent needs a sign-in first.
pub(super) const AUTH_REQUIRED: i64 = -32000;

/// What a connection is for.
#[derive(Clone, Debug)]
pub(in super::super) enum Goal {
    /// A live session: a new one, or one to resume.
    Open(OpenRequest),
    /// The project's past sessions.
    List,
    /// What a past session said, as the agent replays it.
    History(SessionId),
}

/// What a connection for a list or a history found.
#[derive(Debug)]
pub(in super::super) enum Found {
    Sessions(Vec<SessionSummary>),
    History(Vec<Event>),
}

/// How long a mode or model request holds the next message while the agent has not answered it.
pub(super) const SETTINGS_WAIT: Duration = Duration::from_secs(10);

/// atelier's requests waiting for their answer, by what each was.
pub(super) enum Asked {
    Initialize,
    Authenticate,
    Open,
    List,
    Prompt,
    SetMode(String),
    SetModel(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    /// Before the agent has said which session this is.
    Starting,
    Ready,
    Over,
}

/// How many past sessions a list shows, as for Claude Code.
pub(super) const LISTED: usize = 50;

/// The most characters of a title a session row keeps.
pub(super) const TITLE_MAX: usize = 100;
