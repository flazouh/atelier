use super::AccountUsage;

/// The result of a read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsageHistory {
    pub accounts: Vec<AccountUsage>,
    /// Lines, files and folders that could not be read.
    pub skipped: usize,
}
