use crate::handoff_targets::Target;
use crate::providers::Choice;

#[test]
fn an_agent_alone_is_its_backend_name() {
    let target = Target { backend: "codex".into(), provider: None };
    assert_eq!(target.id(), "codex");
    assert_eq!(Target::parse("codex"), Some(target));
}

#[test]
fn an_account_and_openrouter_come_back_as_they_went() {
    for provider in [Choice::Account("work".into()), Choice::OpenRouter, Choice::usual()] {
        let target = Target { backend: "claude-code".into(), provider: Some(provider) };
        assert_eq!(Target::parse(&target.id()), Some(target));
    }
}

#[test]
fn a_provider_this_build_does_not_know_names_nothing() {
    assert_eq!(Target::parse("claude-code/nonsense"), None);
}
