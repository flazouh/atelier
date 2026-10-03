//! A branch's commits and one commit's changes, for the Code lens's History view: what `git log` and
//! `git show` print, read into rows. Pure; the project asks git off the UI thread.

mod helpers;
mod types;

pub use helpers::{log_args, parse_log, show_args, split_show};
pub use types::{Commit, Read, Shown};

#[cfg(test)]
mod tests;
