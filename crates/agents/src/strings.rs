//! The agents' words, one file per key and every language in each file (docs/i18n.md). Written by `atelier-translate`.
mod planning_next_moves;
mod waiting_for_claude;

pub use planning_next_moves::PLANNING_NEXT_MOVES;
pub use waiting_for_claude::WAITING_FOR_CLAUDE;
