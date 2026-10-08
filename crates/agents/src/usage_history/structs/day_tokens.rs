use super::{Day, Tokens};

/// What one model used on one day.
#[derive(Debug, Clone, PartialEq)]
pub struct DayTokens {
    pub day: Day,
    pub model: String,
    pub tokens: Tokens,
}
