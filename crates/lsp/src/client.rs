//! One language server, spawned and spoken to over stdio.
//!
//! A reader thread parses every message the server sends. A reply to one of our requests goes into the
//! pending map and wakes whoever waited for it. Anything the server started itself, a diagnostic or a
//! log, goes on the [`ServerMessage`] channel for the caller to read.

use std::{
    collections::HashMap,
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
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
    ClientCapabilities, Diagnostic, GeneralClientCapabilities, PositionEncodingKind, DiagnosticClientCapabilities, DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentDiagnosticReport, DocumentDiagnosticReportResult, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, InitializeParams, InitializeResult,
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

type Pending = Arc<Mutex<HashMap<i64, Sender<Result<Value, LspError>>>>>;

/// The pipe into the server. The client sends its requests on it, and the read loop its answers.
type Writer<W> = Arc<Mutex<W>>;

pub struct LspClient {
    child: Child,
    stdin: Writer<BufWriter<ChildStdin>>,
    next_id: AtomicI64,
    pending: Pending,
    /// What the server says on its own. The caller drains it.
    pub messages: Receiver<ServerMessage>,
}

impl LspClient {
    /// Starts `program` with `args` and shakes hands for `root`. On success the server is ready for
    /// documents.
    pub fn spawn(
        program: &Path,
        args: &[String],
        root: &Path,
        options: Option<Value>,
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
        let stdin = Arc::new(Mutex::new(BufWriter::new(stdin)));
        thread::spawn({
            let (pending, stdin, root) = (Arc::clone(&pending), Arc::clone(&stdin), root.to_path_buf());
            move || read_loop(BufReader::new(stdout), pending, tx, stdin, root)
        });

        let mut client = Self {
            child,
            stdin,
            next_id: AtomicI64::new(1),
            pending,
            messages,
        };
        let result = client.initialize(root, options, timeout)?;
        Ok((client, result))
    }

    fn initialize(&mut self, root: &Path, options: Option<Value>, timeout: Duration) -> Result<InitializeResult, LspError> {
        let uri = path_to_uri(root)?;
        #[allow(deprecated)] // `root_uri` is the field every server still reads.
        let params = InitializeParams {
            root_uri: Some(uri),
            initialization_options: options,
            capabilities: ClientCapabilities {
                // Saying we pull diagnostics is what makes rust-analyzer answer a pull with its full
                // checks; without it the answer is empty.
                text_document: Some(TextDocumentClientCapabilities {
                    diagnostic: Some(DiagnosticClientCapabilities::default()),
                    // A server that does not answer pulls publishes instead, and some, such as
                    // typescript-language-server, publish only to a client that says it reads them.
                    publish_diagnostics: Some(lsp_types::PublishDiagnosticsClientCapabilities {
                        version_support: Some(true),
                        ..Default::default()
                    }),
                    // A tree, so "Names in this file" can say what each name sits inside.
                    document_symbol: Some(lsp_types::DocumentSymbolClientCapabilities {
                        hierarchical_document_symbol_support: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                // Characters first, as gpui-base counts them; UTF-16, which every server supports,
                // second. The worker converts whatever the server picks (see `encoding`).
                general: Some(GeneralClientCapabilities {
                    position_encodings: Some(vec![PositionEncodingKind::UTF32, PositionEncodingKind::UTF16]),
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

    /// Every place the symbol at `position` is used, without its declaration when
    /// `include_declaration` is false. `None` means the server had no answer.
    pub fn references(
        &mut self,
        path: &Path,
        position: Position,
        include_declaration: bool,
        timeout: Duration,
    ) -> Result<Option<Vec<lsp_types::Location>>, LspError> {
        let params = lsp_types::ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: path_to_uri(path)? },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: lsp_types::ReferenceContext { include_declaration },
        };
        self.request::<lsp_types::request::References>(params, timeout)
    }

    /// What the symbol at `position` is.
    /// The names `path` writes down: its functions, types and fields.
    pub fn document_symbols(
        &mut self,
        path: &Path,
        timeout: Duration,
    ) -> Result<Option<lsp_types::DocumentSymbolResponse>, LspError> {
        let params = lsp_types::DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri: path_to_uri(path)? },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.request::<lsp_types::request::DocumentSymbolRequest>(params, timeout)
    }

    /// Every name in the project that matches `query`, as the server matches it.
    pub fn workspace_symbols(
        &mut self,
        query: &str,
        timeout: Duration,
    ) -> Result<Option<lsp_types::WorkspaceSymbolResponse>, LspError> {
        let params = lsp_types::WorkspaceSymbolParams {
            query: query.to_string(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.request::<lsp_types::request::WorkspaceSymbolRequest>(params, timeout)
    }

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
        let params = PullDiagnosticsParams { text_document: TextDocumentIdentifier { uri: path_to_uri(path)? } };
        match self.request::<PullDiagnostics>(params, timeout)? {
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
        send_on(&self.stdin, body)
    }
}

/// Writes one message whole, so the client's requests and the read loop's answers never interleave.
fn send_on(writer: &Writer<impl Write>, body: &Value) -> Result<(), LspError> {
    let bytes = serde_json::to_vec(body).map_err(|e| LspError::Protocol(e.to_string()))?;
    let mut out = writer.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    write_message(&mut *out, &bytes).map_err(LspError::Transport)?;
    out.flush().map_err(LspError::Transport)
}

impl Drop for LspClient {
    fn drop(&mut self) {
        // A server left running would hold the workspace lock, so it is killed if shutdown never ran.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Reads every message until the server closes, routing replies to the waiters and answering the
/// server's own requests on the spot. Some servers (tsgo) answer nothing until they hear back, while
/// the worker may be blocked waiting on one of those answers, so the answer cannot wait for the worker.
fn read_loop(
    mut input: impl std::io::BufRead,
    pending: Pending,
    out: Sender<ServerMessage>,
    writer: Writer<impl Write>,
    root: PathBuf,
) {
    while let Ok(Some(body)) = read_message(&mut input) {
        let Ok(message) = serde_json::from_slice::<Value>(&body) else { continue };
        match classify(&message) {
            Routed::Request { id, method, params } => {
                let body = match answer_for(&method, &params, &root) {
                    Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                    Err((code, message)) => {
                        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
                    }
                };
                // A failed write means the server is gone, which the end of this loop reports.
                let _ = send_on(&writer, &body);
                if out.send(ServerMessage::Log(format!("answered the server's {method}"))).is_err() {
                    break;
                }
            }
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
    /// A request the server sent, which the read loop answers.
    Request { id: Value, method: String, params: Value },
    /// Something the server said on its own.
    Server(ServerMessage),
    /// A request from the server we do not answer.
    Ignore,
}

/// `textDocument/diagnostic` with only the document. `lsp-types`' own `DocumentDiagnosticParams` writes
/// an unset `identifier` as `null`, which the spec does not allow and tsgo refuses.
enum PullDiagnostics {}

impl Request for PullDiagnostics {
    type Params = PullDiagnosticsParams;
    type Result = DocumentDiagnosticReportResult;
    const METHOD: &'static str = "textDocument/diagnostic";
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullDiagnosticsParams {
    text_document: TextDocumentIdentifier,
}

/// Sorts one message. Pulled out of the read loop so it can be tested without a server.
fn classify(message: &Value) -> Routed {
    // A server request carries an id and a method; only a reply has no method. The server picks the
    // id, and it may be a string (tsgo's are "ts1", "ts2", ...), so it is sent back as it came.
    if let (Some(id), Some(method)) = (message.get("id"), message.get("method").and_then(Value::as_str)) {
        return Routed::Request {
            id: id.clone(),
            method: method.to_string(),
            params: message.get("params").cloned().unwrap_or(Value::Null),
        };
    }
    // Replies answer lathe's own requests, which it numbers.
    if let Some(id) = message.get("id").and_then(Value::as_i64) {
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

/// The file a `file://` URI names, with its percent-escapes decoded. `None` for any other scheme,
/// such as a server's virtual documents. Two servers may escape the same path differently (`@` or
/// `%40`), so paths, never URI strings, are what the worker compares.
pub fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    if uri.scheme().map(|s| s.as_str()) != Some("file") {
        return None;
    }
    let bytes = uri.path().as_str().as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok().map(PathBuf::from)
}

/// JSON-RPC's code for a method the receiver does not implement.
pub const METHOD_NOT_FOUND: i64 = -32601;

/// What lathe answers to a request a server sends. A server that asks for settings gets none, so it
/// uses its defaults; one that registers a capability or a progress token is told yes; one that asks
/// for the workspace folders gets the one root. Anything else is not implemented, which a server must
/// accept rather than hang on an answer that never comes.
pub fn answer_for(method: &str, params: &Value, root: &Path) -> Result<Value, (i64, String)> {
    match method {
        "workspace/configuration" => {
            let items = params.get("items").and_then(Value::as_array).map_or(0, Vec::len);
            Ok(Value::Array(vec![Value::Null; items]))
        }
        "client/registerCapability" | "client/unregisterCapability" | "window/workDoneProgress/create" => {
            Ok(Value::Null)
        }
        "workspace/workspaceFolders" => {
            let uri = path_to_uri(root).map_err(|e| (-32603, e.to_string()))?;
            let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            Ok(json!([{ "uri": uri.as_str(), "name": name }]))
        }
        other => Err((METHOD_NOT_FOUND, format!("lathe does not implement {other}"))),
    }
}
