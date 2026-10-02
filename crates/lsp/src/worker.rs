//! One language server on a thread of its own, so a caller on a UI thread never waits on it.
//!
//! A worker serves every document under one project root. Each request carries the document's path
//! and whole text; the worker opens a path the server has not seen and tells it about text it has not
//! seen, so an answer is always about what the user sees. Answers come back through a callback on the
//! worker's thread.
//!
//! Nothing here names a language. What differs between servers is read from their capabilities:
//! - Diagnostics are pulled (LSP 3.17 `textDocument/diagnostic`) from a server that offers it, which
//!   works them out on the text it has. From one that does not, the newest published set for the
//!   document's current version is taken.
//! - Every position the worker takes or returns counts characters; it converts to and from the
//!   encoding the server chose (see [`crate::encoding`]).
//!
//! The server stops when the last [`LspWorker`] handle is dropped.

mod helpers;
mod structs;
mod types;

pub use helpers::{canonical, until_settled};
pub use structs::{Doc, DocumentSync, LspWorker};
pub use types::{CONTENT_MODIFIED, Reply, SERVER_CANCELLED};

#[cfg(test)]
use helpers::triage;
#[cfg(test)]
use types::Job;

#[cfg(test)]
use std::{path::{Path, PathBuf}, time::Duration};
#[cfg(test)]
use lsp_types::Position;
#[cfg(test)]
use crate::LspError;

#[cfg(test)]
mod tests;
