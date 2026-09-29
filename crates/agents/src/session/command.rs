//! What the app tells a session.
use super::event::{ChoiceId, RequestId};

/// How much an agent may do without asking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PermissionMode {
    /// Ask before anything that changes something.
    Ask,
    AcceptEdits,
    /// Read and plan; change nothing.
    Plan,
    /// The agent decides which actions need a question.
    Auto,
    /// Never ask.
    Bypass,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Send { text: String },
    Answer { request: RequestId, choice: ChoiceId },
    Interrupt,
    SetModel { model: String },
    SetPermissionMode { mode: PermissionMode },
}
