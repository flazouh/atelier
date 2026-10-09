use std::path::PathBuf;

use atelier_capabilities::Actor;

/// The person using the app: the user of this machine.
pub(super) fn person_actor() -> Actor {
    let name = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "me".into());
    Actor::person(name.clone(), name)
}

/// The agent of a session, working for `person`. Every session has this one identity: the app does not tell one Claude
/// Code session from another in the tracker, and a person's agent never has more rights than the person.
pub(super) fn agent_actor(person: &Actor) -> Actor {
    Actor::agent("claude-code", "Claude Code", person.id.clone())
}

/// The folder for config files: `run` beside the settings file, so a test that points `ATELIER_SETTINGS` elsewhere
/// writes elsewhere.
pub(super) fn run_dir() -> Option<PathBuf> {
    atelier_settings::path()?
        .parent()
        .map(|dir| dir.join("run"))
}

/// `wanted` when no local provider has it as its account, else `wanted-2`, `wanted-3`. Two projects in folders of one
/// name must not share an account, or one would hide the other.
pub(super) fn unique_account(taken: &[String], wanted: &str) -> String {
    let free = |name: &str| !taken.iter().any(|t| t == name);
    if free(wanted) {
        return wanted.to_string();
    }
    (2..)
        .map(|n| format!("{wanted}-{n}"))
        .find(|name| free(name))
        .unwrap_or_else(|| wanted.to_string())
}
