//! What a session tells the app. Every event is data the UI can draw without knowing the agent.
use std::time::Duration;

use serde_json::Value;

use super::command::PermissionMode;

macro_rules! text_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

text_id!(
    /// A session's id, the one a backend takes back to resume it.
    SessionId
);
text_id!(
    /// A tool call's id; a subagent has the id of the call that started it.
    ToolId
);
text_id!(
    /// A permission request's id.
    RequestId
);
text_id!(
    /// One choice of a permission request.
    ChoiceId
);

/// One streamed block of text or thinking. Deltas of a block share its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub u64);

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

#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub id: ToolId,
    /// The tool's name in the agent's own words.
    pub name: String,
    pub kind: ToolKind,
    /// The arguments, as the agent gave them. `Null` until they are known.
    pub input: Value,
    /// The file the call names, when it names one.
    pub file: Option<String>,
    /// The subagent that made the call, when one did.
    pub parent: Option<ToolId>,
    pub status: ToolStatus,
}

/// What a tool returned. `text` holds at most [`ToolOutput::MAX_TEXT`] bytes; a longer output keeps a
/// head and says so, with the place of the whole output when the agent kept it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolOutput {
    pub text: String,
    pub is_error: bool,
    pub truncated: bool,
    /// Where the whole output lives on the host, when the agent saved it.
    pub full_at: Option<String>,
}

impl ToolOutput {
    pub const MAX_TEXT: usize = 64 * 1024;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subagent {
    /// The id of the call that started it.
    pub id: ToolId,
    pub task: String,
    /// The kind of subagent, in the agent's words.
    pub kind: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Todo {
    pub id: String,
    pub text: String,
    pub status: TodoStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceKind {
    Allow,
    /// Allow now and stop asking for the same thing.
    AllowAlways,
    Deny,
}

/// One answer a permission request offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub id: ChoiceId,
    pub label: String,
    pub kind: ChoiceKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PermissionRequest {
    pub id: RequestId,
    pub call: ToolCall,
    /// The agent's own line about what it wants to do.
    pub reason: Option<String>,
    pub choices: Vec<Choice>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cost_usd: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnOutcome {
    Completed,
    Interrupted,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnEnd {
    pub outcome: TurnOutcome,
    /// The agent's closing text, when it gives one.
    pub summary: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndReason {
    /// lathe closed the session.
    Closed,
    /// The agent's process ended. `None` means a signal ended it.
    Exited(Option<i32>),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Started {
    pub session: SessionId,
    pub model: Option<String>,
    pub mode: Option<PermissionMode>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Started(Started),
    /// A user message the session did not receive from lathe: history, or another client.
    UserMessage { text: String },
    /// Assistant text, streamed. Deltas of one block arrive in order.
    Text { block: BlockId, delta: String },
    /// Thinking, streamed. Some agents send no text; the event still says the agent thinks.
    Thinking { block: BlockId, delta: String },
    ThinkingDone { block: BlockId, took: Duration },
    ToolStarted(ToolCall),
    /// The whole input of a call announced before its input was known, and the file it names.
    ToolInput { id: ToolId, input: Value, file: Option<String> },
    ToolStatus { id: ToolId, status: ToolStatus },
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
    TurnEnded(TurnEnd),
    /// Something went wrong that did not end the session, such as a line that did not parse.
    Warning(String),
    /// The session is over; no event follows.
    Ended(EndReason),
}
