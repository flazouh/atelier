//! A small LSP client. It speaks to one language server over stdio and answers three questions:
//! what is wrong with this file, where is this symbol defined, and what is this symbol.
//!
//! What it does: `initialize`, `initialized`, `textDocument/didOpen`, `didChange`, `didSave`,
//! `shutdown`, `exit`; it reads `textDocument/publishDiagnostics` and asks `textDocument/definition`
//! and `textDocument/hover`.
//!
//! What it does not do yet: completion, rename, code actions, formatting, workspace symbols. It never
//! guesses an answer a server did not give.

pub mod client;
pub mod framing;

pub use client::{DEFAULT_SETTLE, LspClient, LspError, ServerMessage};
