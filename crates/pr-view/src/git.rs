//! The pull request's git, through the project. A pull request's commits, files and file texts come from
//! git, not from the forge: the forge lists the changed files, but only git has the text of both sides. The
//! objects come from a **cache repository** atelier keeps per forge repository, made with `git clone --bare
//! --shared` from the project (so it borrows the project's objects) and fed by `git fetch` of
//! `refs/pull/N/head`. The project's own refs and working tree are never touched. The language server
//! needs real files, so a **checkout** of the head, made with `git archive`, sits beside the cache. Never
//! `git worktree`: a machine may refuse it, and a worktree belongs to the repository it came from.
//!
//! Everything goes through [`Project::spawn`](atelier_project::Project::spawn), so a remote project keeps its cache and checkouts on its
//! own host. Arguments are passed as arguments, never joined into a shell line, except in the one script
//! that pipes `git archive` into `tar`, which reads its values from positional parameters.

mod helpers;
mod structs;
mod types;

pub use helpers::{is_sha, parse_batch, parse_commits, parse_files, remote_for, short};
pub use structs::{Commit, FileEntry, PrGit, Prepared};
pub use types::{Blob, GitError, GitResult, LEGACY_DATA, MAX_TEXT};
