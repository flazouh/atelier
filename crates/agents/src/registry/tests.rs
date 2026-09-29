use super::*;

#[test]
fn the_default_agent_comes_first_and_is_found_by_its_backend() {
    let all = agents();
    assert!(!all.is_empty());
    let first = &all[0];
    let found = by_backend(first.backend.name()).expect("found by its backend's name");
    assert_eq!(found.name, first.name);
    assert!(by_backend("no-such-backend").is_none());
    assert!(first.lab.mark().is_some(), "its models wear a lab mark");
}

#[test]
fn every_agent_has_a_name_a_look_and_capabilities() {
    for agent in agents() {
        assert!(!agent.name.is_empty());
        assert!(!agent.look.labels.waiting.is_empty(), "{} has its words", agent.name);
        let caps = agent.backend.capabilities();
        assert!(!caps.models.is_empty() || !caps.permission_modes.is_empty(), "{} says what it can do", agent.name);
    }
}

#[test]
fn a_model_id_maps_to_its_lab() {
    for id in ["opus", "sonnet", "haiku", "claude-sonnet-4-5", "anthropic/claude-opus-4"] {
        assert_eq!(model_lab(id), Some(Lab::Anthropic), "{id}");
    }
    for id in ["gpt-5.2", "GPT-4o", "o3", "o4-mini", "openai/gpt-5"] {
        assert_eq!(model_lab(id), Some(Lab::OpenAi), "{id}");
    }
    assert_eq!(model_lab("grok-4"), Some(Lab::Xai));
    assert_eq!(model_lab("local-7b"), None);
    assert!(model_mark("opus").is_some());
    assert!(model_mark("grok-4").is_none(), "xAI takes the monogram");
    assert!(model_mark("local-7b").is_none());
}
