use atelier_agents::registry::Agent;
use atelier_agents::session::Account;
use atelier_project::Project;
use atelier_settings::secrets::{OPENROUTER_KEY, Secrets};
use atelier_ui::menu::Branch;

use crate::providers::Choice;

use super::super::structs::{AgentChoices, Targets};

impl Targets {
    /// Asks each of `agents` for its accounts on the project's host, and the keychain for the OpenRouter key.
    /// Blocks: keep it off the UI thread.
    pub fn read(agents: &[Agent], project: &dyn Project, secrets: &dyn Secrets) -> Self {
        let key_kept = matches!(secrets.read(OPENROUTER_KEY), Ok(Some(_)));
        Self { agents: agents.iter().map(|agent| choices_of(agent, project, key_kept)).collect() }
    }

    /// The tree a handoff menu shows: one row for each agent, and for an agent with a choice of providers a menu of them.
    pub fn branches(&self) -> Vec<Branch> {
        self.agents.iter().map(AgentChoices::branch).collect()
    }
}

fn choices_of(agent: &Agent, project: &dyn Project, key_kept: bool) -> AgentChoices {
    let runs_on_providers = agent.backend.capabilities().providers;
    let accounts: Vec<Account> = if runs_on_providers { agent.backend.accounts(project).unwrap_or_default() } else { Vec::new() };
    let mut choices: Vec<Choice> = accounts.iter().filter(|account| account.signed_in).map(|account| Choice::Account(account.name.clone())).collect();
    if runs_on_providers && key_kept {
        choices.push(Choice::OpenRouter);
    }
    AgentChoices { backend: agent.backend.name().into(), name: agent.name.into(), choices, accounts }
}
