//! lathe's own model of an agent session. The app and the UI see only this: events that come out,
//! commands that go in, and a [`Backend`] that starts sessions. Nothing here names an agent, a lab or a
//! wire format, and nothing assumes a child process. A backend is anything that takes commands and
//! yields events: a subprocess with its own protocol (`claude_code`), an ACP agent, or an agent loop
//! that runs inside lathe. `docs/agents.md` says how each one fits.
mod backend;
mod coalesce;
mod command;
mod event;
#[cfg(test)]
mod fake;

pub use backend::{
    Backend, Capabilities, EventSink, ModelChoice, OpenRequest, Session, SessionError, SessionSummary,
};
pub use coalesce::EventQueue;
pub use command::{Command, PermissionMode};
pub use event::{
    BlockId, Choice, ChoiceId, ChoiceKind, EndReason, Event, PermissionRequest, RequestId, SessionId, Started,
    Subagent, Todo, TodoStatus, ToolCall, ToolId, ToolKind, ToolOutput, ToolStatus, TurnEnd, TurnOutcome, Usage,
};

#[cfg(test)]
mod tests;
