use atelier_agents::session::Account;
use atelier_ui::menu::{Branch, Lead};

use crate::handoff_targets::{AgentChoices, Targets};
use crate::providers::Choice;

fn agent(backend: &str, name: &str, choices: Vec<Choice>) -> AgentChoices {
    let accounts = choices
        .iter()
        .filter_map(|choice| match choice {
            Choice::Account(name) => Some(Account { name: name.clone(), plan: Some("max".into()), signed_in: true, email: None }),
            Choice::OpenRouter => None,
        })
        .collect();
    AgentChoices { backend: backend.into(), name: name.into(), mark: None, choices, accounts }
}

fn branches(agents: Vec<AgentChoices>) -> Vec<Branch> {
    Targets { agents }.branches()
}

#[test]
fn an_agent_with_no_choice_is_one_row_that_hands_off_at_once() {
    assert_eq!(branches(vec![agent("codex", "Codex", vec![])]), vec![Branch::leaf("codex", "Codex").led(Lead::Monogram)]);
}

#[test]
fn an_agent_with_one_provider_is_one_row_that_names_it() {
    let only = agent("claude-code", "Claude Code", vec![Choice::usual()]);
    let rows = branches(vec![only]);
    assert_eq!(rows, vec![Branch::leaf(format!("claude-code/{}", Choice::usual().key()), "Claude Code").led(Lead::Monogram)]);
}

#[test]
fn an_agent_with_several_providers_opens_a_menu_of_them() {
    let many = agent("claude-code", "Claude Code", vec![Choice::usual(), Choice::Account("work".into()), Choice::OpenRouter]);
    let rows = branches(vec![many]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "claude-code");
    let labels: Vec<_> = rows[0].branches.iter().map(|b| b.label.to_string()).collect();
    assert_eq!(labels, ["Max", "Max · work", "OpenRouter"]);
    let ids: Vec<_> = rows[0].branches.iter().map(|b| b.id.to_string()).collect();
    assert_eq!(ids, [format!("claude-code/{}", Choice::usual().key()), "claude-code/account:work".to_string(), "claude-code/openrouter".to_string()]);
}

#[test]
fn every_row_leads_with_a_mark_or_a_monogram_the_agent_and_each_provider() {
    let many = agent("claude-code", "Claude Code", vec![Choice::usual(), Choice::OpenRouter]);
    let rows = branches(vec![many, agent("codex", "Codex", vec![])]);
    assert_eq!(rows[0].lead, Some(Lead::Monogram), "an agent with no mark gets its monogram");
    assert_eq!(rows[1].lead, Some(Lead::Monogram));
    assert!(rows[0].branches.iter().all(|provider| matches!(provider.lead, Some(Lead::Mark(_)))), "a provider shows its lab's mark");
}
