use std::{
    io::{BufReader, BufWriter, Write},
    path::Path,
    sync::{Arc, Mutex, atomic::{AtomicI64, Ordering}, mpsc::{self, Receiver, RecvTimeoutError}},
    thread,
    time::Duration,
};

use lsp_types::{
    ClientCapabilities, Diagnostic, DiagnosticClientCapabilities, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentDiagnosticReport,
    DocumentDiagnosticReportResult, GeneralClientCapabilities, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, InitializeParams, InitializeResult,
    PartialResultParams, Position, PositionEncodingKind, TextDocumentClientCapabilities,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, VersionedTextDocumentIdentifier, WorkDoneProgressParams,
    request::Request,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use atelier_project::{Command, Control, Project};

use super::types::{LspError, Pending, PullDiagnostics, ServerMessage, Writer};
use super::helpers::{path_to_uri, read_loop, send_on};

pub struct LspClient {
    pub(super) control: Box<dyn Control>,
    stdin: Writer<BufWriter<Box<dyn Write + Send>>>,
    next_id: AtomicI64,
    pub(super) pending: Pending,
    /// What the server says on its own. The caller drains it.
    pub messages: Receiver<ServerMessage>,
}

impl LspClient {
    /// Starts `program` with `args` in `root`, through `project`, so the server runs where the project
    /// lives, and shakes hands for `root`. On success the server is ready for documents.
    pub fn spawn(
        project: &dyn Project,
        program: &Path,
        args: &[String],
        root: &Path,
        options: Option<Value>,
        timeout: Duration,
    ) -> Result<(Self, InitializeResult), LspError> {
        let process = project.spawn(&Command::new(program).args(args.iter().cloned()).cwd(root)).map_err(LspError::Spawn)?;
        let (stdin, stdout, control) = (process.stdin, process.stdout, process.control);
        let pending: Pending = Arc::default();
        let (tx, messages) = mpsc::channel();
        let stdin = Arc::new(Mutex::new(BufWriter::new(stdin)));
        thread::spawn({
            let (pending, stdin, root) = (Arc::clone(&pending), Arc::clone(&stdin), root.to_path_buf());
            move || read_loop(BufReader::new(stdout), pending, tx, stdin, root)
        });

        let mut client = Self {
            control,
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
        let _ = self.control.wait();
        Ok(())
    }

    pub(super) fn request<R: Request>(&mut self, params: R::Params, timeout: Duration) -> Result<R::Result, LspError>
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

    pub(super) fn send(&mut self, body: &Value) -> Result<(), LspError> {
        send_on(&self.stdin, body)
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        // A server left running would hold the workspace lock, so it is killed if shutdown never ran.
        let _ = self.control.kill();
        let _ = self.control.wait();
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PullDiagnosticsParams {
    pub(super) text_document: TextDocumentIdentifier,
}
