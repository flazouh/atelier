//! A session's short title, drafted by its agent after the first turn, in place of the first prompt
//! cut short. The reader's own name for the session always wins.

mod helpers;
mod types;

pub use helpers::{clean, prompt};
#[cfg(test)]
pub use types::TITLE_MOST;

#[cfg(test)]
mod tests;
