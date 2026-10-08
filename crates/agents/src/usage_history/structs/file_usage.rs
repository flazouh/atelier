use super::Tokens;

/// Tokens of one model in one 15 minute bucket (`bucket * BUCKET_SECS` is the start in epoch seconds).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Rec {
    pub(crate) bucket: i64,
    pub(crate) model: String,
    pub(crate) tokens: Tokens,
}

/// Everything one log file says, before any day or range is applied.
#[derive(Debug, Default)]
pub(crate) struct FileUsage {
    pub(crate) session_id: String,
    pub(crate) subagent: bool,
    pub(crate) cwd: Option<String>,
    /// A title the log names (Claude's custom or AI title).
    pub(crate) title: Option<String>,
    pub(crate) first_prompt: Option<String>,
    pub(crate) records: Vec<Rec>,
    /// Lines that could not be read.
    pub(crate) skipped: usize,
}
