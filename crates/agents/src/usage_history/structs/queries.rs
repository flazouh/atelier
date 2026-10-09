use super::{Day, Tokens};
use crate::usage_history::types::Provider;

/// Tokens of one day, summed over the chosen accounts.
#[derive(Debug, Clone, PartialEq)]
pub struct DayTotal {
    pub day: Day,
    pub tokens: Tokens,
    /// An estimate; `None` when a model of that day has no price.
    pub est_cost_usd: Option<f64>,
}

/// The days of one account.
#[derive(Debug, Clone, PartialEq)]
pub struct AccountDays {
    pub provider: Provider,
    pub label: String,
    pub days: Vec<DayTotal>,
}

/// Tokens of one model over a range.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelTotal {
    pub model: String,
    pub tokens: Tokens,
    pub est_cost_usd: Option<f64>,
}

/// A session with only the tokens inside a range.
#[derive(Debug, Clone, PartialEq)]
pub struct RangedSession {
    pub account: String,
    pub provider: Provider,
    pub id: String,
    pub title: String,
    pub project: String,
    pub model_main: String,
    pub tokens: Tokens,
    pub est_cost_usd: Option<f64>,
    pub last_active_secs: i64,
}
