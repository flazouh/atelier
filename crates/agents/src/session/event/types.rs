use std::time::Duration;

use serde_json::Value;

use super::structs::{
    BlockId, ContextFill, FileEdit, PermissionRequest, RequestId, Started, Subagent, Todo, ToolCall, ToolId,
    ToolOutput, TurnEnd, Usage,
};

/// What a tool does, so the UI can pick a look. The tool's own name stays in [`ToolCall::name`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolKind {
    Read,
    Edit,
    Write,
    Search,
    Shell,
    Fetch,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolStatus {
    /// Announced; its input may still stream in, or it waits for a permission.
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceKind {
    Allow,
    /// Allow now and stop asking for the same thing.
    AllowAlways,
    Deny,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnOutcome {
    Completed,
    Interrupted,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndReason {
    /// atelier closed the session.
    Closed,
    /// The agent's process ended. `code` is `None` when a signal ended it; `stderr` is the last lines it
    /// wrote, which say why when it stopped early.
    Exited { code: Option<i32>, stderr: String },
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Started(Started),
    /// A user message the session did not receive from atelier: history, or another client.
    UserMessage { text: String },
    /// Assistant text, streamed. Deltas of one block arrive in order.
    Text { block: BlockId, delta: String },
    /// Thinking, streamed. Some agents send no text; the event still says the agent thinks.
    Thinking { block: BlockId, delta: String },
    ThinkingDone { block: BlockId, took: Duration },
    ToolStarted(ToolCall),
    /// The file a call will touch, as soon as the stream names it: before the input is whole, and so
    /// before the tool runs. A review takes the text of the file before its edit lands.
    ToolTarget { id: ToolId, file: String },
    /// The whole input of a call announced before its input was known, and the file it names.
    ToolInput { id: ToolId, input: Value, file: Option<String> },
    /// The text of an edit or a write, as far as it is known: told again as it grows while the agent writes the call, and
    /// when the call is whole. Each telling replaces the last.
    ToolEdit { id: ToolId, edit: FileEdit },
    ToolStatus { id: ToolId, status: ToolStatus },
    /// What a call does, when an update shows more than its start did: an edit that makes a new file is a write.
    ToolKind { id: ToolId, kind: ToolKind },
    ToolFinished { id: ToolId, output: ToolOutput },
    SubagentStarted(Subagent),
    SubagentProgress { id: ToolId, activity: String },
    SubagentEnded { id: ToolId, ok: bool, summary: Option<String> },
    /// The whole todo list, each time it changes.
    Todos(Vec<Todo>),
    Permission(PermissionRequest),
    /// The agent withdrew a request, or a turn ended before it was answered.
    PermissionCancelled(RequestId),
    Usage(Usage),
    /// How full the context is, each time it changes.
    Context(ContextFill),
    TurnEnded(TurnEnd),
    /// Something went wrong that did not end the session, such as a line that did not parse.
    Warning(String),
    /// The session is over; no event follows.
    Ended(EndReason),
}
