//! GitHub records who wrote a comment as the account of `gh`, so an agent's comment would read as the person's. The
//! provider adds a hidden line to an agent's comment and reads it back: the line holds the actor, with who it works
//! for. A person's comment gets no line.
use atelier_capabilities::{Actor, ActorKind};

const OPEN: &str = "<!-- atelier-actor: ";
const CLOSE: &str = " -->";

/// `body` with the hidden line, when `by` is an agent.
pub fn with_actor(body: &str, by: &Actor) -> String {
    if by.kind != ActorKind::Agent {
        return body.to_string();
    }
    match serde_json::to_string(by) {
        Ok(json) => format!("{body}\n\n{OPEN}{json}{CLOSE}"),
        Err(_) => body.to_string(),
    }
}

/// The text of a comment as written, and the actor of its hidden line when it has one that reads.
pub fn split(body: &str) -> (String, Option<Actor>) {
    let trimmed = body.trim_end();
    if let Some(start) = trimmed.rfind(OPEN)
        && trimmed.ends_with(CLOSE)
        && let Ok(actor) =
            serde_json::from_str::<Actor>(&trimmed[start + OPEN.len()..trimmed.len() - CLOSE.len()])
    {
        return (trimmed[..start].trim_end().to_string(), Some(actor));
    }
    (body.to_string(), None)
}
