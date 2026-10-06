use super::super::event::{ChoiceId, RequestId};

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

/// The pictures a message can carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl ImageFormat {
    pub fn media_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }

    /// The format a file name ends in, if it is one of these.
    pub fn of_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "gif" => Some(Self::Gif),
            "webp" => Some(Self::Webp),
            _ => None,
        }
    }
}

/// Something a message carries beside its text, in atelier's terms. A backend that takes only text writes
/// it out with [`Attachment::render`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Attachment {
    /// A comment on lines of a file, with the lines quoted so the agent reads what was meant even if the
    /// file has moved on. `removed` says the lines are ones the agent removed, counted in the file as it
    /// was before the turn.
    LineComment { path: String, first_line: u32, last_line: u32, removed: bool, quote: String, body: String },
    /// A file the message is about.
    File { path: String },
    /// Text that was pasted, kept apart from what was written.
    Text { text: String },
    /// Words from the conversation the reader replied to, and what they wrote about them (which may be nothing).
    Quote { quote: String, note: String },
    /// A picture.
    Image { format: ImageFormat, bytes: std::sync::Arc<[u8]> },
}

impl Attachment {
    /// The attachment as text, for a backend that carries only text.
    pub fn render(&self) -> String {
        match self {
            Self::LineComment { path, first_line, last_line, removed, quote, body } => {
                let lines = if first_line == last_line { format!("line {first_line}") } else { format!("lines {first_line}-{last_line}") };
                let quoted: String = quote.lines().map(|line| format!("> {line}\n")).collect();
                let which = if *removed { " (lines the agent removed, numbered as they were before)" } else { "" };
                format!("Review comment on {path}, {lines}{which}:\n{quoted}{body}")
            }
            Self::File { path } => format!("File: {path}"),
            Self::Text { text } => {
                // The fence is longer than any run of backticks inside, so the text cannot close it early.
                let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
                let fence = "`".repeat((longest + 1).max(3));
                format!("Pasted text:\n{fence}\n{text}\n{fence}")
            }
            Self::Quote { quote, note } => {
                let quoted: String = quote.lines().map(|line| format!("> {line}\n")).collect();
                if note.trim().is_empty() { format!("Quoting:\n{quoted}") } else { format!("Quoting:\n{quoted}\n{note}") }
            }
            Self::Image { format, bytes } => {
                format!("An image was attached ({}, {} KB), which this agent cannot read.", format.media_type(), bytes.len() / 1024)
            }
        }
    }

    /// The picture, for a backend that can carry one.
    pub fn image(&self) -> Option<(ImageFormat, &[u8])> {
        match self {
            Self::Image { format, bytes } => Some((*format, bytes)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Send { text: String, attachments: Vec<Attachment> },
    Answer { request: RequestId, choice: ChoiceId },
    /// The reader's answers to an agent's questions: each question's text, and the label (or labels, joined by a comma) picked.
    AnswerQuestions { request: RequestId, answers: Vec<(String, String)> },
    Interrupt,
    SetModel { model: String },
    SetPermissionMode { mode: PermissionMode },
    /// Asks what fills the context window. The answer comes as [`Event::ContextParts`](crate::session::Event::ContextParts);
    /// an agent that cannot break the window down refuses.
    RefreshContext,
}

impl Command {
    /// A message with no attachments.
    pub fn send(text: impl Into<String>) -> Self {
        Self::Send { text: text.into(), attachments: Vec::new() }
    }
}
