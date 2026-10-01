//! The commit of what a review kept (`kept.rs`). It is built on a temporary index, so the reader's own
//! staged work is neither taken nor lost: the index starts from `HEAD` (empty in a repository with no
//! commit yet) and takes each kept file's text as a blob, then `git commit -F -` commits it, so the
//! repository's pre-commit and commit-msg hooks run and `commit.gpgsign` holds. Only then does the
//! reader's index take the committed blobs, for those paths alone. A hook that refuses stops everything
//! and changes nothing.
//!
//! The branch must not move under it: HEAD is read first, read again just before `git commit`, and the
//! commit's parent is checked after it, so a commit the reader made meanwhile is never undone.
//!
//! The same dozen git calls whatever the number of files, since each is a round trip on a remote
//! project: one `ls-tree` for the modes, one `fast-import` stream for all the blobs, one
//! `update-index --index-info` per index. Blocking: call it off the UI thread.

mod helpers;
mod structs;
mod types;

pub use helpers::commit;
pub(crate) use helpers::{git, run};
#[cfg(test)]
pub(crate) use helpers::commit_with;
#[cfg(test)]
pub use types::CommitError;

#[cfg(test)]
use atelier_project::Project;

#[cfg(test)]
mod tests;
