//! atelier's own model of an agent session. The app and the UI see only this: events that come out,
//! commands that go in, and a [`Backend`] that starts sessions. Nothing here names an agent, a lab or a
//! wire format, and nothing assumes a child process. A backend is anything that takes commands and
//! yields events: a subprocess with its own protocol (`claude_code`), an ACP agent, or an agent loop
//! that runs inside atelier. `docs/agents.md` says how each one fits.
mod backend;
mod coalesce;
mod command;
mod conversation;
mod event;
#[cfg(test)]
mod fake;

pub use backend::{
    Account, ApiKey, Backend, Capabilities, EventSink, ModelChoice, OpenRequest, Provider, Session, SessionError, SessionSummary,
};
pub use coalesce::EventQueue;
pub use conversation::{Answer, Call, Conversation, Item, SubagentStatus};
pub use command::{Attachment, Command, PermissionMode, message_text};
pub use event::{
    BlockId, Choice, ChoiceId, ChoiceKind, ContextFill, ContextPart, EndReason, Event, FileEdit, Limit, LimitState, LimitWindow, PermissionRequest, RequestId, SessionId, Started,
    Subagent, Todo, TodoStatus, ToolCall, ToolId, ToolKind, ToolOutput, ToolStatus, TurnEnd, TurnOutcome, Usage,
};

#[cfg(test)]
mod tests;
