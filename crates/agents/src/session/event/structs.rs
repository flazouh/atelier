use serde_json::Value;

use super::super::command::PermissionMode;
use super::types::{ChoiceKind, LimitState, LimitWindow, TodoStatus, ToolKind, ToolStatus, TurnOutcome};

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

/// The text an edit changes, in the same words for every agent: the file, the text it replaces and the text it puts in
/// its place. A write that makes or replaces a whole file has no old text. While the agent still writes the call, each
/// text holds what has come so far.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileEdit {
    pub path: String,
    pub old: String,
    pub new: String,
}

/// One streamed block of text or thinking. Deltas of a block share its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub u64);

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

    /// At most [`Self::MAX_TEXT`] bytes of `text`, cut on a character boundary and marked `truncated`
    /// when cut.
    pub fn head(text: &str, is_error: bool) -> Self {
        let cut = (0..=Self::MAX_TEXT.min(text.len())).rev().find(|i| text.is_char_boundary(*i)).unwrap_or(0);
        Self { text: text[..cut].to_string(), is_error, truncated: cut < text.len(), full_at: None }
    }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Todo {
    pub id: String,
    pub text: String,
    pub status: TodoStatus,
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

/// How full the agent's context window is: the tokens its latest request carried, and the window's
/// size once the agent has told it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContextFill {
    pub used: u64,
    pub window: Option<u64>,
}

/// Where the account stands against its plan's usage limit, as the agent last told it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit {
    pub state: LimitState,
    /// When the limit's window starts again, in seconds since the Unix epoch.
    pub resets_at: Option<u64>,
    pub window: Option<LimitWindow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnEnd {
    pub outcome: TurnOutcome,
    /// The agent's closing text, when it gives one.
    pub summary: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Started {
    pub session: SessionId,
    pub model: Option<String>,
    pub mode: Option<PermissionMode>,
    /// The agent's own slash commands, by name without the slash, for the composer to offer.
    pub commands: Vec<String>,
}
