//! Opening the pull request: the bases to pick from, the prompt the agent drafts the title and the
//! body from, and the draft's split into the two. Blocking: call it off the UI thread.

mod helpers;
mod types;

pub use helpers::{ahead, bases, prompt, title_and_body};

#[cfg(test)]
mod tests;
