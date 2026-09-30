//! Tests of our own agent. The model clients run against a local server that plays scripted streams (see
//! `server`), so nothing here calls a real API or needs a key.
mod anthropic;
mod context;
mod draft;
mod http;
mod openai;
mod permission;
mod server;
mod session;
mod sse;
mod store;
mod support;
mod tools;

#[test]
fn every_fallback_model_is_a_known_id_and_only_haiku_wants_a_thinking_budget() {
    const KNOWN: [&str; 5] = ["claude-opus-5-5", "claude-fable-5-1", "claude-sonnet-5-5", "claude-sonnet-5", "claude-haiku-4-5"];
    let models = super::anthropic_models();
    assert!(models.len() >= 4);
    for m in &models {
        assert!(KNOWN.contains(&m.id.as_str()), "{} is not a known model id", m.id);
        assert_eq!(super::anthropic::wants_budget(&m.id), m.id.starts_with("claude-haiku-4-5"), "{}", m.id);
    }
    let ids: std::collections::HashSet<_> = models.iter().map(|m| m.id.clone()).collect();
    assert_eq!(ids.len(), models.len(), "no id twice");
}
