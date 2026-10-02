#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// Estimated tokens the conversation may hold.
    pub limit_tokens: usize,
    /// The newest messages are never shortened.
    pub keep_recent: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self { limit_tokens: 120_000, keep_recent: 8 }
    }
}

/// What a compaction did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Compaction {
    pub shortened: usize,
    pub saved_bytes: usize,
}
