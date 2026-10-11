use atelier_bots::Harness;

use super::super::agent_of;
use super::super::helpers::backend_of;

#[test]
fn a_harness_the_app_runs_has_its_agent_and_the_others_have_none() {
    assert_eq!(agent_of(Harness::ClaudeCode).map(|a| a.name), Some("Claude Code"));
    assert_eq!(agent_of(Harness::Codex).map(|a| a.name), Some("Codex"));
    assert_eq!(agent_of(Harness::Cursor).map(|a| a.name), Some("Cursor"));
    for harness in [Harness::Grok, Harness::Opencode, Harness::Custom] {
        assert!(agent_of(harness).is_none(), "{harness:?}");
    }
}

/// A backend the map names is one the registry has, so a rename there fails here.
#[test]
fn every_backend_the_map_names_is_in_the_registry() {
    for harness in [Harness::ClaudeCode, Harness::Codex, Harness::Cursor] {
        let backend = backend_of(harness).expect("the harness has a backend");
        assert!(atelier_agents::registry::by_backend(backend).is_some(), "{backend}");
    }
}

/// Every starter can start: its harness is one the app runs.
#[test]
fn every_starter_bot_has_an_agent() {
    for bot in atelier_bots::starter_crew() {
        assert!(agent_of(bot.harness).is_some(), "{}", bot.id);
    }
}
