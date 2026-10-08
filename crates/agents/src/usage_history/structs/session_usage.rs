use super::{DayTokens, Tokens};

/// One session: its tokens by day and model.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionUsage {
    pub id: String,
    pub title: String,
    /// The last component of the working folder.
    pub project: String,
    /// The model with the most tokens.
    pub model_main: String,
    /// Sorted by day, then model. Only days from the `since` day on.
    pub days: Vec<DayTokens>,
    pub totals: Tokens,
    /// An estimate; `None` when a model of the session has no price.
    pub est_cost_usd: Option<f64>,
    /// Epoch seconds of the last use, to 15 minutes.
    pub last_active_secs: i64,
}
