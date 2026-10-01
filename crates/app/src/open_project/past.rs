//! The agent's past sessions as the sidebar lists them: never a session that is open, which shows once,
//! as the open one; and the last activity of each, which gives an open session its place and its stamp.
use atelier_agents::session::{SessionId, SessionSummary};

/// `past` without the sessions whose ids are in `open`.
pub fn not_open(past: Vec<SessionSummary>, open: &[SessionId]) -> Vec<SessionSummary> {
    past.into_iter().filter(|p| !open.contains(&p.id)).collect()
}

/// When `id` was last active, as `list` says.
pub fn last_activity(list: &[SessionSummary], id: &SessionId) -> Option<u64> {
    list.iter().find(|p| p.id == *id).and_then(|p| p.updated)
}

#[cfg(test)]
mod tests;
