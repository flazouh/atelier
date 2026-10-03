//! A branch's commits and one commit's changes, for the Code lens's History view, and what the checkout
//! holds uncommitted, for its Changes view: what `git log`, `git show` and `git diff` print, read into rows.
//! Pure; the project asks git off the UI thread.

mod helpers;
mod types;

pub use helpers::{EMPTY_TREE, UNTRACKED_ARGS, diff_args, log_args, new_file_args, parse_log, show_args, split_patch, split_show};
pub use types::{Commit, CommitFile, Read, Shown};

#[cfg(test)]
mod tests;
