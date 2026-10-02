//! Which tool calls run at once, which wait for an answer, and which never run. Pure: the mode, what the
//! call does, and the rules the reader added decide. This is atelier's own logic; no backend supplies it.

mod helpers;
mod structs;
mod types;

pub use helpers::decide;
pub use structs::Rules;
pub use types::Verdict;
