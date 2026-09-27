//! One language server, spawned and spoken to over stdio.
//!
//! A reader thread parses every message the server sends. A reply to one of our requests goes into the
//! pending map and wakes whoever waited for it. Anything the server started itself, a diagnostic or a
//! log, goes on the [`ServerMessage`] channel for the caller to read.

use std::{
    collections::HashMap,
    io::{BufReader, BufWriter, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    thread,
    time::Duration,
};

use lsp_types::{
    ClientCapabilities, Diagnostic, DiagnosticClientCapabilities, DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, InitializeParams, InitializeResult,
    PartialResultParams, Position, PublishDiagnosticsParams, TextDocumentClientCapabilities, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams, Uri, VersionedTextDocumentIdentifier,
    WorkDoneProgressParams, request::Request,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::framing::{read_message, write_message};

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

type Pending = Arc<Mutex<HashMap<i64, Sender<Result<Value, LspError>>>>>;

pub struct LspClient {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    next_id: AtomicI64,
    pending: Pending,
    /// What the server says on its own. The caller drains it.
    pub messages: Receiver<ServerMessage>,
}

impl LspClient {
    /// Starts `program` with `args` and shakes hands for `root`. On success the server is ready for
    /// documents.
    pub fn spawn(
        program: &str,
        args: &[&str],
        root: &Path,
        timeout: Duration,
    ) -> Result<(Self, InitializeResult), LspError> {
        let mut child = Command::new(program)
            .args(args)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(LspError::Spawn)?;
        let stdout = child.stdout.take().expect("stdout was piped");
        let stdin = child.stdin.take().expect("stdin was piped");
        let pending: Pending = Arc::default();
        let (tx, messages) = mpsc::channel();
        thread::spawn({
            let pending = Arc::clone(&pending);
            move || read_loop(BufReader::new(stdout), pending, tx)
        });

        let mut client = Self {
            child,
            stdin: BufWriter::new(stdin),
            next_id: AtomicI64::new(1),
            pending,
            messages,
        };
        let result = client.initialize(root, timeout)?;
        Ok((client, result))
    }

    fn initialize(&mut self, root: &Path, timeout: Duration) -> Result<InitializeResult, LspError> {
        let uri = path_to_uri(root)?;
        #[allow(deprecated)] // `root_uri` is the field every server still reads.
        let params = InitializeParams {
            root_uri: Some(uri),
            capabilities: ClientCapabilities {
                // Saying we pull diagnostics is what makes rust-analyzer answer a pull with its full
                // checks; without it the answer is empty.
                text_document: Some(TextDocumentClientCapabilities {
                    diagnostic: Some(DiagnosticClientCapabilities::default()),
                    ..Default::default()
                }),
                // rust-analyzer then says when it has loaded the workspace. Other servers ignore it.
                experimental: Some(json!({ "serverStatusNotification": true })),
                ..Default::default()
            },
            ..Default::default()
        };
        let result: InitializeResult = self.request::<lsp_types::request::Initialize>(params, timeout)?;
        self.notify::<lsp_types::notification::Initialized>(lsp_types::InitializedParams {})?;
        Ok(result)
    }

    /// Tells the server about a file and its text.
    pub fn did_open(&mut self, path: &Path, language_id: &str, version: i32, text: &str) -> Result<(), LspError> {
        self.notify::<lsp_types::notification::DidOpenTextDocument>(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: path_to_uri(path)?,
                language_id: language_id.to_string(),
                version,
                text: text.to_string(),
            },
        })
    }

    /// Replaces a file's whole text. Whole-file changes keep the client simple and are what the server
    /// needs anyway after an agent rewrites a region.
    pub fn did_change(&mut self, path: &Path, version: i32, text: &str) -> Result<(), LspError> {
        self.notify::<lsp_types::notification::DidChangeTextDocument>(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri: path_to_uri(path)?, version },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: text.to_string(),
            }],
        })
    }

    pub fn did_save(&mut self, path: &Path) -> Result<(), LspError> {
        self.notify::<lsp_types::notification::DidSaveTextDocument>(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: path_to_uri(path)? },
            text: None,
        })
    }

    /// Where the symbol at `position` is defined. `None` means the server had no answer, which is not
    /// an error: nothing is made up in its place.
    pub fn definition(
        &mut self,
        path: &Path,
        position: Position,
        timeout: Duration,
    ) -> Result<Option<GotoDefinitionResponse>, LspError> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: path_to_uri(path)? },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.request::<lsp_types::request::GotoDefinition>(params, timeout)
    }

    /// What the symbol at `position` is.
    pub fn hover(&mut self, path: &Path, position: Position, timeout: Duration) -> Result<Option<Hover>, LspError> {
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: path_to_uri(path)? },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.request::<lsp_types::request::HoverRequest>(params, timeout)
    }

    /// What is wrong with `path` now, worked out by the server when asked (LSP 3.17 pull diagnostics).
    /// Unlike a published set, the answer is always about the text the server has at that moment, so
    /// there is nothing to wait out. rust-analyzer answers with its own checks; `cargo check` results
    /// still arrive only as published sets.
    pub fn pull_diagnostics(&mut self, path: &Path, timeout: Duration) -> Result<Vec<Diagnostic>, LspError> {
        let params = DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri: path_to_uri(path)? },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        match self.request::<lsp_types::request::DocumentDiagnosticRequest>(params, timeout)? {
            DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(report)) => {
                Ok(report.full_document_diagnostic_report.items)
            }
            // We never name an earlier report, so "unchanged" or a partial answer means the server did
            // not follow the protocol.
            DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Unchanged(_)) => {
                Err(LspError::Protocol("the server said unchanged, but no earlier report was named".into()))
            }
            DocumentDiagnosticReportResult::Partial(_) => {
                Err(LspError::Protocol("the server sent a partial report nobody asked for".into()))
            }
        }
    }

    /// Waits for the next set of diagnostics for `path`, up to `timeout`. Sets for other files are
    /// dropped, so a caller watching one file is not woken by its neighbours.
    pub fn wait_for_diagnostics(
        &self,
        path: &Path,
        timeout: Duration,
    ) -> Result<PublishDiagnosticsParams, LspError> {
        self.wait_for_diagnostics_at(path, None, timeout, DEFAULT_SETTLE)
    }

    /// What the server most recently says is wrong with `path` at document version `version`.
    ///
    /// A server publishes more than once for the same text: rust-analyzer sends an empty set while it
    /// indexes, then the real one. An empty set is also how a server says a file is clean, so nothing
    /// in the protocol tells the two apart. This waits for the first set that matches, then keeps
    /// taking later matching sets until `settle` passes with none, and returns the last. That is the
    /// server's settled answer rather than its first guess.
    pub fn wait_for_diagnostics_at(
        &self,
        path: &Path,
        version: Option<i32>,
        timeout: Duration,
        settle: Duration,
    ) -> Result<PublishDiagnosticsParams, LspError> {
        let want = path_to_uri(path)?;
        let deadline = std::time::Instant::now() + timeout;
        let mut best: Option<PublishDiagnosticsParams> = None;
        loop {
            let left = match &best {
                // Nothing yet: wait for the whole deadline.
                None => deadline.saturating_duration_since(std::time::Instant::now()),
                // We have an answer: wait only for a better one.
                Some(_) => settle.min(deadline.saturating_duration_since(std::time::Instant::now())),
            };
            if left.is_zero() {
                return best.ok_or(LspError::Timeout);
            }
            match self.messages.recv_timeout(left) {
                Ok(ServerMessage::Diagnostics(params))
                    if fresh_enough(&params.uri, params.version, &want, version) =>
                {
                    best = Some(params);
                }
                Ok(ServerMessage::Exited) => return best.ok_or(LspError::Closed),
                Ok(_) => continue,
                Err(RecvTimeoutError::Timeout) => return best.ok_or(LspError::Timeout),
                Err(RecvTimeoutError::Disconnected) => return best.ok_or(LspError::Closed),
            }
        }
    }

    /// Asks the server to stop, then waits for it.
    pub fn shutdown(mut self, timeout: Duration) -> Result<(), LspError> {
        let _ = self.request::<lsp_types::request::Shutdown>((), timeout);
        let _ = self.notify::<lsp_types::notification::Exit>(());
        let _ = self.child.wait();
        Ok(())
    }

    fn request<R: Request>(&mut self, params: R::Params, timeout: Duration) -> Result<R::Result, LspError>
    where
        R::Params: serde::Serialize,
        R::Result: DeserializeOwned,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.pending.lock().expect("the pending map is not poisoned").insert(id, tx);
        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": R::METHOD,
            "params": params,
        });
        self.send(&body)?;
        let value = match rx.recv_timeout(timeout) {
            Ok(result) => result?,
            Err(RecvTimeoutError::Timeout) => {
                self.pending.lock().expect("the pending map is not poisoned").remove(&id);
                return Err(LspError::Timeout);
            }
            Err(RecvTimeoutError::Disconnected) => return Err(LspError::Closed),
        };
        serde_json::from_value(value).map_err(|e| LspError::Protocol(e.to_string()))
    }

    fn notify<N: lsp_types::notification::Notification>(&mut self, params: N::Params) -> Result<(), LspError>
    where
        N::Params: serde::Serialize,
    {
        let body = json!({ "jsonrpc": "2.0", "method": N::METHOD, "params": params });
        self.send(&body)
    }

    fn send(&mut self, body: &Value) -> Result<(), LspError> {
        let bytes = serde_json::to_vec(body).map_err(|e| LspError::Protocol(e.to_string()))?;
        write_message(&mut self.stdin, &bytes).map_err(LspError::Transport)?;
        self.stdin.flush().map_err(LspError::Transport)
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        // A server left running would hold the workspace lock, so it is killed if shutdown never ran.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Reads every message until the server closes, routing replies to the waiters.
fn read_loop(mut input: impl std::io::BufRead, pending: Pending, out: Sender<ServerMessage>) {
    while let Ok(Some(body)) = read_message(&mut input) {
        let Ok(message) = serde_json::from_slice::<Value>(&body) else { continue };
        match classify(&message) {
            Routed::Reply(id, result) => {
                if let Some(waiter) = pending.lock().expect("the pending map is not poisoned").remove(&id) {
                    let _ = waiter.send(result);
                }
            }
            Routed::Server(message) => {
                if out.send(message).is_err() {
                    break;
                }
            }
            Routed::Ignore => {}
        }
    }
    let _ = out.send(ServerMessage::Exited);
}

/// What one message from the server is.
enum Routed {
    /// A reply to the request with this id.
    Reply(i64, Result<Value, LspError>),
    /// Something the server started.
    Server(ServerMessage),
    /// A request from the server we do not answer.
    Ignore,
}

/// Sorts one message. Pulled out of the read loop so it can be tested without a server.
fn classify(message: &Value) -> Routed {
    if let Some(id) = message.get("id").and_then(Value::as_i64) {
        // A server request also carries a method; only a reply has none.
        if message.get("method").is_some() {
            return Routed::Ignore;
        }
        if let Some(error) = message.get("error") {
            let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
            let text = error.get("message").and_then(Value::as_str).unwrap_or("no reason given");
            return Routed::Reply(id, Err(LspError::Server { code, message: text.to_string() }));
        }
        let result = message.get("result").cloned().unwrap_or(Value::Null);
        return Routed::Reply(id, Ok(result));
    }
    match message.get("method").and_then(Value::as_str) {
        Some("textDocument/publishDiagnostics") => {
            match serde_json::from_value(message.get("params").cloned().unwrap_or(Value::Null)) {
                Ok(params) => Routed::Server(ServerMessage::Diagnostics(params)),
                Err(_) => Routed::Ignore,
            }
        }
        Some("experimental/serverStatus") => {
            match message.get("params").and_then(|p| p.get("quiescent")).and_then(Value::as_bool) {
                Some(quiescent) => Routed::Server(ServerMessage::Status { quiescent }),
                None => Routed::Ignore,
            }
        }
        Some("window/logMessage") | Some("window/showMessage") => {
            let text = message
                .get("params")
                .and_then(|p| p.get("message"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            Routed::Server(ServerMessage::Log(text))
        }
        _ => Routed::Ignore,
    }
}

/// Whether one published set answers what the caller asked for: the right file, and the version we
/// sent or a later one. A server that sends no version is taken at its word, since there is nothing
/// better to go on. Pulled out so the staleness rule can be tested without a server.
fn fresh_enough(uri: &Uri, published: Option<i32>, want_uri: &Uri, want_version: Option<i32>) -> bool {
    if uri != want_uri {
        return false;
    }
    match (want_version, published) {
        (None, _) => true,
        (Some(_), None) => true,
        (Some(want), Some(published)) => published >= want,
    }
}

/// A file path as the `file://` URI every server expects.
pub fn path_to_uri(path: &Path) -> Result<Uri, LspError> {
    let path = path.canonicalize().map_err(LspError::Transport)?;
    let text = path.to_str().ok_or_else(|| LspError::Protocol("a path that is not UTF-8".into()))?;
    let mut encoded = String::from("file://");
    for byte in text.bytes() {
        match byte {
            // Unreserved, RFC 3986 section 2.3.
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~'
            // Sub-delims, plus the two extra path characters, RFC 3986 section 3.3. A server built
            // on Rust's `url` crate leaves every one of these raw. We compare URIs byte for byte,
            // so escaping one here means the server's diagnostics never match the file we asked
            // about: a path holding `+` waited out the full timeout while the answer sat unread.
            | b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'=' | b':'
            | b'@' | b'/' => encoded.push(byte as char),
            // Everything else: space, `#`, `?`, `%`, the control bytes, and every non-ASCII byte.
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded.parse().map_err(|_| LspError::Protocol(format!("a path that is not a URI: {text}")))
}

#[cfg(test)]
mod tests;
