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

/// Something a message carries beside its text, in lathe's terms. A backend that takes only text writes
/// it out with [`Attachment::render`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Attachment {
    /// A comment on lines of a file, with the lines quoted so the agent reads what was meant even if the
    /// file has moved on.
    LineComment { path: String, first_line: u32, last_line: u32, quote: String, body: String },
    /// A file the message is about.
    File { path: String },
}

impl Attachment {
    /// The attachment as text, for a backend that carries only text.
    pub fn render(&self) -> String {
        match self {
            Self::LineComment { path, first_line, last_line, quote, body } => {
                let lines = if first_line == last_line { format!("line {first_line}") } else { format!("lines {first_line}-{last_line}") };
                let quoted: String = quote.lines().map(|line| format!("> {line}\n")).collect();
                format!("Review comment on {path}, {lines}:\n{quoted}{body}")
            }
            Self::File { path } => format!("File: {path}"),
        }
    }
}

/// `text`, then each attachment, apart by a blank line.
pub fn message_text(text: &str, attachments: &[Attachment]) -> String {
    let mut out = text.to_string();
    for attachment in attachments {
        out.push_str("\n\n");
        out.push_str(&attachment.render());
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Send { text: String, attachments: Vec<Attachment> },
    Answer { request: RequestId, choice: ChoiceId },
    Interrupt,
    SetModel { model: String },
    SetPermissionMode { mode: PermissionMode },
}

impl Command {
    /// A message with no attachments.
    pub fn send(text: impl Into<String>) -> Self {
        Self::Send { text: text.into(), attachments: Vec::new() }
    }
}
