//! The Ship strip, under the review bar: what the review kept, as one commit. Closed, it offers Commit
//! (⌘⇧C). Open, it lists what the commit takes against HEAD, file by file with +N −M (the text before
//! the turn can hold the reader's own uncommitted edits, and those go in too), asks for a new branch on
//! the default branch, and holds the message; Commit is ⌘↵. The agent drafts the branch name and the
//! message (`Backend::draft`); the reader edits both, and a draft never replaces what they typed. Every
//! git call and draft runs off the UI thread.

mod helpers;
mod structs;
mod types;

pub use helpers::bind_keys;
pub use structs::ShipStrip;
#[cfg(test)]
pub use structs::Line;
pub use types::{Stage, StripEvent};

#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use gpui_kit::Entity;
#[cfg(test)]
use atelier_project::Project;
#[cfg(test)]
use crate::ship::kept::Kept;

#[cfg(test)]
mod tests;
