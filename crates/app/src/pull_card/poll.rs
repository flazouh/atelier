//! How soon the card reads the pull request again: often while checks run, less once they settle, never
//! after a merge or a close. A failure backs off, and a rate limit waits as long as the forge says.

mod helpers;
mod types;

pub use helpers::next_delay;
pub use types::Seen;

#[cfg(test)]
mod tests;
