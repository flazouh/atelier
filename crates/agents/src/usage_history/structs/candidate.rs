use std::path::PathBuf;
use std::time::SystemTime;

use crate::usage_history::types::Provider;

/// A log file worth opening.
pub(crate) struct Candidate {
    pub(crate) account: usize,
    pub(crate) provider: Provider,
    pub(crate) path: PathBuf,
    pub(crate) mtime: SystemTime,
    pub(crate) size: u64,
    /// The session this file belongs to, when its name does not say (a subagent file).
    pub(crate) session_hint: Option<String>,
    pub(crate) subagent: bool,
}
