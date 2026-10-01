use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc::{Sender}},
    time::Duration,
};

use lsp_types::{DocumentDiagnosticReportResult, PublishDiagnosticsParams, request::Request};
use serde_json::Value;

use super::structs::PullDiagnosticsParams;

/// How long to keep listening for a better set of diagnostics once one has arrived. rust-analyzer's
/// empty indexing set and its real answer land within a few hundred milliseconds of each other.
pub const DEFAULT_SETTLE: Duration = Duration::from_millis(600);

/// What went wrong. Each one says which side failed, so a caller can tell a missing server from a
/// server that answered badly.
#[derive(Debug)]
pub enum LspError {
    /// The server could not be started. Usually it is not installed.
    Spawn(std::io::Error),
    /// The pipe to the server broke.
    Transport(std::io::Error),
    /// The server sent something we could not read.
    Protocol(String),
    /// The server answered with an error of its own.
    Server { code: i64, message: String },
    /// The server did not answer inside the deadline.
    Timeout,
    /// The server has already stopped.
    Closed,
    /// A newer request made this one moot, so it was never sent.
    Superseded,
}

impl std::fmt::Display for LspError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(e) => write!(f, "the language server did not start: {e}"),
            Self::Transport(e) => write!(f, "the pipe to the language server broke: {e}"),
            Self::Protocol(m) => write!(f, "the language server sent something unreadable: {m}"),
            Self::Server { code, message } => write!(f, "the language server refused: {message} ({code})"),
            Self::Timeout => write!(f, "the language server did not answer in time"),
            Self::Closed => write!(f, "the language server has stopped"),
            Self::Superseded => write!(f, "a newer request replaced this one"),
        }
    }
}

impl std::error::Error for LspError {}

/// Something the server said on its own.
#[derive(Debug, Clone)]
pub enum ServerMessage {
    /// What is wrong with one file, as the server now sees it. A later set replaces an earlier one.
    Diagnostics(PublishDiagnosticsParams),
    /// A log or status line, for the status row.
    Log(String),
    /// Whether the server has finished loading and checking (rust-analyzer's
    /// `experimental/serverStatus`). Before it says `true`, its answers can be empty for want of a
    /// loaded workspace rather than because nothing is there.
    Status { quiescent: bool },
    /// The server stopped.
    Exited,
}

pub(super) type Pending = Arc<Mutex<HashMap<i64, Sender<Result<Value, LspError>>>>>;

/// The pipe into the server. The client sends its requests on it, and the read loop its answers.
pub(super) type Writer<W> = Arc<Mutex<W>>;

/// What one message from the server is.
pub(super) enum Routed {
    /// A reply to the request with this id.
    Reply(i64, Result<Value, LspError>),
    /// A request the server sent, which the read loop answers.
    Request { id: Value, method: String, params: Value },
    /// Something the server said on its own.
    Server(ServerMessage),
    /// A request from the server we do not answer.
    Ignore,
}

/// `textDocument/diagnostic` with only the document. `lsp-types`' own `DocumentDiagnosticParams` writes
/// an unset `identifier` as `null`, which the spec does not allow and tsgo refuses.
pub(super) enum PullDiagnostics {}

impl Request for PullDiagnostics {
    type Params = PullDiagnosticsParams;
    type Result = DocumentDiagnosticReportResult;
    const METHOD: &'static str = "textDocument/diagnostic";
}

/// JSON-RPC's code for a method the receiver does not implement.
pub const METHOD_NOT_FOUND: i64 = -32601;
