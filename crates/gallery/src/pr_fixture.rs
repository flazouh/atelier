//! The Pull request story's repository: a small Rust crate written to disk, so rust-analyzer answers
//! hover, definition, uses and names on it, and the pull request's changes over it. Plain data: the
//! files at the head commit, and for each changed file the rows the change removed.

mod helpers;
mod structs;
mod types;

pub use helpers::list_files;
pub use structs::Fixture;

#[cfg(test)]
use helpers::shown;
#[cfg(test)]
use types::{ABORT_TEST, CHANGES, REQUEST};

#[cfg(test)]
mod tests;
