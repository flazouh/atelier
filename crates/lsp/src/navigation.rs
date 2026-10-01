//! Go to definition, then references, as Zed does: a definition answer that is empty, or that is only
//! the symbol under the caret, turns into the places the symbol is used. Everything here is pure, so
//! the rule is tested without a server.

mod helpers;
mod structs;
mod types;

pub use helpers::{definition_links, lands_on_itself, location_link, sort_targets};
pub use structs::{Navigation, Target};
pub use types::Found;

#[cfg(test)]
use std::path::{Path, PathBuf};
#[cfg(test)]
use lsp_types::{GotoDefinitionResponse, Location, LocationLink, Position, Range, Uri};

#[cfg(test)]
mod tests;
