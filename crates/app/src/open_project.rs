//! One project open in the window: its tree, its git branch, its tabs and their buffers, and the
//! language servers for its files. Everything that touches the project runs on a background thread
//! through [`Project`](atelier_project::Project); this entity only holds what came back, so switching projects is instant.
//!
//! A file that changes on disk reloads when its tab is clean. When the tab holds unsaved edits, the
//! tab keeps them and says the file changed, with Reload and Keep mine. A file deleted on disk says
//! so, with Close and Keep; a kept one holds its text as unsaved, and a save asks before it creates
//! the file again.

mod chips;
pub mod front;
mod helpers;
mod past;
mod structs;
mod types;

pub use structs::OpenProject;
pub use types::{Deleted, Listing, ProjectEvent};
#[cfg(test)]
pub use types::Git;
pub(crate) use types::NO_FORGE_REMOTE;

#[cfg(test)]
use gpui_kit::{Context, Entity, Window, base::input::Position};
#[cfg(test)]
use atelier_editor::Jump;
#[cfg(test)]
use atelier_project::{Change, ChangeKind, Project, Watch};
#[cfg(test)]
use atelier_settings::Location;

#[cfg(test)]
mod tests;
