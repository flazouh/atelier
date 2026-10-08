use std::path::PathBuf;

use crate::usage_history::types::Provider;

/// One folder to read: a Claude config folder (holds `projects/`) or a Codex home (holds `sessions/`).
#[derive(Debug, Clone, PartialEq)]
pub struct AccountRoot {
    pub provider: Provider,
    pub label: String,
    pub dir: PathBuf,
}

/// The accounts to read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Roots {
    pub(crate) accounts: Vec<AccountRoot>,
}
