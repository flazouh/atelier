//! The pull request view on real data (M5). Three layers:
//!
//! - **Data** ([`data`], [`load`], [`snapshot`]): one pull request as the forge tells it, read in parts
//!   that arrive as they finish, and kept on disk so the next open paints at once.
//! - **Git** ([`git`], [`diff`]): the pull request's commits and files from a cache repository and a
//!   checkout of the head, both made through `Project::git` and `Project::spawn` (never `git worktree`),
//!   so a remote project works as a local one.
//! - **State and views** ([`state`], [`sync`], and the views): what the reader has seen, live refresh with
//!   a backoff, the list of pull requests and the pull request itself.
//!
//! Every call that blocks runs off the UI thread. `docs/pr-view.md` says how the parts fit.
pub mod base;
pub mod data;
pub mod diff;
pub mod fixture;
pub mod git;
pub mod load;
pub mod snapshot;
pub mod state;

pub use data::{Part, PartKind, PullData};

#[cfg(test)]
mod tests;
