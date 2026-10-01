//! Pushing the branch to `origin`, and pulling with a rebase when the remote moved on. Never forced.
//! Git never waits on a login prompt: with none, a push that needs one fails and says so. Blocking:
//! call it off the UI thread.

mod helpers;
mod structs;
mod types;

pub use helpers::{pull_rebase, pull_rebase_setting_aside, push};
#[cfg(test)]
pub use helpers::{batch_ssh, classify};
pub(crate) use helpers::remote_git;
#[cfg(test)]
pub(crate) use helpers::pull_rebase_setting_aside_with;
pub use types::{PushError, PutBack, RebaseError};
#[cfg(test)]
pub use types::ENTRY_NAME;

#[cfg(test)]
mod tests;
