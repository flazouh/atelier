use atelier_agents::session::Account;

use crate::providers::Choice;

/// One agent and the providers the reader can use with it. Empty for an agent that runs on none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentChoices {
    /// The agent's backend name.
    pub backend: String,
    /// What a person calls it.
    pub name: String,
    pub choices: Vec<Choice>,
    /// The accounts the choices name, for their labels.
    pub accounts: Vec<Account>,
}
