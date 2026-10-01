//! A project's identity for the tracker: where its database lives, and the prefix of its short ids.

mod helpers;
mod types;

pub use helpers::prefix_for;
pub use types::ProjectKey;

#[cfg(test)]
mod tests;
