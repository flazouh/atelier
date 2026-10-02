//! Which language server serves which file. This is the only place that names a language or a
//! server: everything else takes a [`ServerSpec`] and works the same for all of them. Adding a
//! language is adding a row to [`LANGUAGES`] and, if no server already covers it, one to [`SERVERS`].

mod helpers;
mod structs;
mod types;

pub use helpers::{find_program, find_program_in, find_root, language_id, search_dirs, server_for};
pub use structs::ServerSpec;
pub use types::{LANGUAGES, NODE, SERVERS};

#[cfg(test)]
use std::path::Path;
#[cfg(test)]
use crate::install::Kind;

#[cfg(test)]
mod tests;
