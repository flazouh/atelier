use crate::providers::Choice;

/// Where one session is handed off to: an agent, and the provider it runs on when the reader chose one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// The agent's backend name.
    pub backend: String,
    /// `None` for an agent that has no choice of provider, or to keep the default one.
    pub provider: Option<Choice>,
}
