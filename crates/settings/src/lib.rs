//! What atelier remembers across launches: the theme in force, whether it follows the system, the primary
//! colour the reader picked, and the recent projects. One small JSON
//! file in the app's data folder (`<data dir>/atelier/settings.json`), shared by the app and the gallery.
//! It is read once before the first window opens, and changed on a background thread, so the UI
//! thread never waits on the disk.
//!
//! A change loads the file, changes it and writes it back ([`update`]), and keys this version does not
//! know are kept, so the gallery setting the theme never drops the app's recent projects.

mod helpers;
pub mod secrets;
mod structs;
mod types;

pub use helpers::{load, path, update};
pub use structs::{AgentModels, OpenSession, Panels, Settings, SidebarSaved, WhatsNew};
pub use types::{Location, RECENT_LIMIT};


#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
mod tests;
