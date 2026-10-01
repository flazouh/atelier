//! A file's language server, wired into the editor the way Zed does it: hold ⌘ (ctrl on Linux) over
//! a symbol to underline it and click to go to its definition, or to its uses when it is the
//! definition; F12 does the same from the caret and ⇧F12 lists the uses; rest the pointer on a symbol
//! for its hover card; problems are checked on open and again whenever typing pauses.
//!
//! Nothing here names a language: the file's path picks the server from `atelier_lsp::servers`, and
//! [`LspWorker`] runs it on a thread of its own, so no answer ever holds the window.
//!
//! A pull request's diff shows removed rows the file does not have. Its session holds a [`RowMap`]:
//! the server reads the file without them, and every row in this file is mapped across at this
//! boundary, so everything else here counts shown rows. A jump into another file goes to the owner
//! ([`Elsewhere`]) when there is one; otherwise the status line names the place.

mod helpers;
mod structs;
mod types;

pub use helpers::{file_name, go_to_definition};
pub use structs::{EditorSession, Jump};
pub use types::{ASK, Elsewhere, READY};

#[cfg(test)]
use helpers::summary;
#[cfg(test)]
use structs::Rows;

#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use atelier_ui::RowMap;
#[cfg(test)]
use lsp_types::Position;

#[cfg(test)]
mod tests;
