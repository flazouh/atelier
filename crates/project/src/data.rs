//! A project's data folder: atelier's own files about a project, such as its agents' sessions and its
//! reviews, on the project's host and outside the repository, so they never show in `git status`.
//! `<data>/atelier/projects/<folder>-<hash of the root>/`, where `<data>` is the platform's data folder
//! (`~/Library/Application Support` on macOS, `$XDG_DATA_HOME` or `~/.local/share` elsewhere), or
//! `ATELIER_DATA_DIR`. The same root always gets the same folder.

mod helpers;
mod structs;
mod types;

pub use helpers::adopt_old_data;
pub use structs::{DataEntry, DataFolder};
