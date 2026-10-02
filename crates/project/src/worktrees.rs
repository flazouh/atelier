//! A project's worktrees and what each one holds: what `git worktree list` says, and in each worktree its
//! uncommitted files, its commits found nowhere else and whether it was merged, so a worktree can be shown
//! and removed knowing what would be lost. Everything goes through [`Project::git`](crate::Project::git),
//! so a remote project's worktrees read the same.

mod helpers;
mod structs;

pub use helpers::worktrees;
#[cfg(test)]
use helpers::parse_list;
pub use structs::{TreeState, Upstream, Worktree};

#[cfg(test)]
mod tests;
