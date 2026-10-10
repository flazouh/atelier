use serde::{Deserialize, Serialize};

/// One thing a bot, a workspace or a project remembers.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MemoryNote {
    /// Counts up inside one layer. A removed number is not used again.
    pub id: u64,
    pub text: String,
    /// When it was written, in milliseconds since 1970.
    pub created_ms: i64,
}
