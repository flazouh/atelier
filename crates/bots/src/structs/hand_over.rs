use super::BotId;

/// What one bot wrote for the steps after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandOver {
    pub bot: BotId,
    pub text: String,
}
