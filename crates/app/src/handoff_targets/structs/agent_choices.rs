use atelier_agents::session::Account;
use atelier_ui::BrandMark;

use crate::providers::Choice;

/// One agent and the providers the reader can use with it. Empty for an agent that runs on none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentChoices {
    /// The agent's backend name.
    pub backend: String,
    /// What a person calls it.
    pub name: String,
    /// Its mark, or `None` for a monogram.
    pub mark: Option<BrandMark>,
    pub choices: Vec<Choice>,
    /// The accounts the choices name, for their labels.
    pub accounts: Vec<Account>,
}
