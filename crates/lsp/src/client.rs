//! One language server, spawned and spoken to over stdio.
//!
//! A reader thread parses every message the server sends. A reply to one of our requests goes into the
//! pending map and wakes whoever waited for it. Anything the server started itself, a diagnostic or a
//! log, goes on the [`ServerMessage`] channel for the caller to read.

mod helpers;
mod structs;
mod types;

pub use helpers::{answer_for, path_to_uri, uri_to_path};
pub use structs::LspClient;
pub use types::{DEFAULT_SETTLE, LspError, METHOD_NOT_FOUND, ServerMessage};

#[cfg(test)]
use helpers::{classify, read_loop};
#[cfg(test)]
use structs::PullDiagnosticsParams;
#[cfg(test)]
use types::{Pending, Routed};

#[cfg(test)]
use std::sync::{Arc, Mutex, mpsc};
#[cfg(test)]
use lsp_types::{TextDocumentIdentifier, Uri};
#[cfg(test)]
use serde_json::{Value, json};

#[cfg(test)]
mod tests;
