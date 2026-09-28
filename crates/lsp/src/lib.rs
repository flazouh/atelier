//! A small LSP client. It speaks to one language server over stdio and answers: what is wrong with
//! this file, where is this symbol defined, where is it used, what is it, and which names a file or
//! the project writes down.
//!
//! What it does: `initialize`, `initialized`, `textDocument/didOpen`, `didChange`, `didSave`,
//! `shutdown`, `exit`; it reads `textDocument/publishDiagnostics` and asks `textDocument/definition`,
//! `references`, `hover`, `documentSymbol` and `workspace/symbol`.
//!
//! What it does not do yet: completion, rename, code actions, formatting. It never guesses an answer a
//! server did not give.

pub mod client;
pub mod encoding;
pub mod framing;
pub mod install;
pub mod navigation;
pub mod pool;
pub mod published;
pub mod servers;
pub mod symbols;
pub mod worker;

pub use client::{DEFAULT_SETTLE, LspClient, LspError, ServerMessage};
pub use install::{Launch, Store, Unavailable};
pub use navigation::{Found, Navigation, Target, definition_links};
pub use pool::{NoServer, Workers};
pub use symbols::Symbol;
pub use servers::{LANGUAGES, SERVERS, ServerSpec, find_program, find_root, search_dirs, language_id, server_for};
pub use worker::{CONTENT_MODIFIED, Doc, DocumentSync, LspWorker, Reply, SERVER_CANCELLED, canonical, until_settled};
