//! An ACP agent keeps its own sessions, so atelier asks it for them: a short connection that lists the
//! project's sessions (`session/list`) or replays one (`session/load`), then stops the agent. It runs
//! through the project, so a remote project's sessions are read on its host.

mod helpers;
mod types;

pub(super) use helpers::{history, list};
