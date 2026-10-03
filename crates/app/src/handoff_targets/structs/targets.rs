use super::AgentChoices;

/// Every agent the project offers, with its providers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Targets {
    pub(in super::super) agents: Vec<AgentChoices>,
}
