pub const DEFAULT_BASE: &str = "https://api.anthropic.com";

pub(super) const VERSION: &str = "2023-06-01";

/// How much of an error body is read: enough for its message.
pub(super) const ERROR_BODY: usize = 8 * 1024;

pub(super) enum Partial {
    Text(String),
    Thinking { text: String, signature: Option<String> },
    Tool { id: String, name: String, json: String },
    /// A block type this client does not use (server tools, for one). Its deltas are read and dropped.
    Skip,
}
