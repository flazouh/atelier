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

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use lsp_types::{Diagnostic, Hover, Position, PublishDiagnosticsParams, Range, Uri};

use crate::{
    DEFAULT_SETTLE, LspClient, LspError, ServerMessage,
    client::path_to_uri,
    encoding::{Encoding, range_from_server, to_server},
    navigation::{Found, Navigation, Target, definition_links, lands_on_itself, sort_targets},
    servers::{ServerSpec, language_id},
};

/// Where an answer goes. It runs on the worker's thread.
pub type Reply<T> = Box<dyn FnOnce(Result<T, LspError>) + Send>;

/// A document as the editor has it now.
#[derive(Clone, Debug)]
pub struct Doc {
    pub path: PathBuf,
    pub text: String,
}

/// What the server has been told about one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSync {
    version: i32,
    sent: String,
}

impl DocumentSync {
    /// A document the server opened at version 1 with `text`.
    pub fn opened(text: impl Into<String>) -> Self {
        Self { version: 1, sent: text.into() }
    }

    /// The version to send `text` as, or `None` when the server already has it. A server publishes
    /// nothing for a change that carries the same text, so an unchanged buffer is never sent again.
    pub fn next(&self, text: &str) -> Option<i32> {
        (text != self.sent).then_some(self.version + 1)
    }

    /// Records that the server now has `text` at `version`. Call it only once the send succeeded, or
    /// the next request would skip text the server never got.
    pub fn sent(&mut self, text: &str, version: i32) {
        self.version = version;
        self.sent = text.to_string();
    }

    /// The version of the text the server last got.
    pub fn version(&self) -> i32 {
        self.version
    }
}

enum Job {
    Navigate { doc: Doc, position: Position, reply: Reply<Navigation> },
    References { doc: Doc, position: Position, reply: Reply<Vec<Target>> },
    Hover { doc: Doc, position: Position, reply: Reply<Option<Hover>> },
    Diagnostics { doc: Doc, reply: Reply<Vec<Diagnostic>> },
}

impl Job {
    /// Answers the job with `error` instead of running it.
    fn fail(self, error: LspError) {
        match self {
            Job::Navigate { reply, .. } => reply(Err(error)),
            Job::References { reply, .. } => reply(Err(error)),
            Job::Hover { reply, .. } => reply(Err(error)),
            Job::Diagnostics { reply, .. } => reply(Err(error)),
        }
    }
}

/// A cheap handle to the thread that owns the server. Clone it for each place that asks.
#[derive(Clone)]
pub struct LspWorker {
    jobs: Sender<Job>,
    root: PathBuf,
}

impl LspWorker {
    /// Starts `program` as the server `spec` describes, for the project at `root`, and moves it onto
    /// its own thread once it has shaken hands. It blocks for the handshake, so call it off the UI
    /// thread. Returns the name the server gave. `ready` bounds the handshake, `ask` each request.
    pub fn start(
        spec: &ServerSpec,
        program: &Path,
        root: PathBuf,
        ready: Duration,
        ask: Duration,
    ) -> Result<(Self, String), LspError> {
        let options = (spec.initialization_options)(program, &root);
        let (client, init) = LspClient::spawn(&program.to_string_lossy(), spec.args, &root, options, ready)?;
        let name = init.server_info.map(|i| i.name).unwrap_or_else(|| spec.name.to_string());
        let capabilities = init.capabilities;
        let session = Session {
            client,
            ask,
            encoding: Encoding::negotiated(capabilities.position_encoding.as_ref()),
            pulls: capabilities.diagnostic_provider.is_some(),
            documents: HashMap::new(),
            published: HashMap::new(),
            quiescent: None,
            exited: false,
        };
        let (jobs, queue) = mpsc::channel();
        thread::spawn(move || run(session, queue));
        Ok((Self { jobs, root }, name))
    }

    /// The project this worker serves.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where a Cmd-click at `position` goes: the definition, or the symbol's uses when the caret is
    /// already on its declaration.
    pub fn navigate(&self, doc: Doc, position: Position, reply: Reply<Navigation>) {
        self.send(Job::Navigate { doc, position, reply });
    }

    /// Every place the symbol at `position` is used, its declaration left out.
    pub fn references(&self, doc: Doc, position: Position, reply: Reply<Vec<Target>>) {
        self.send(Job::References { doc, position, reply });
    }

    /// What the symbol at `position` is.
    pub fn hover(&self, doc: Doc, position: Position, reply: Reply<Option<Hover>>) {
        self.send(Job::Hover { doc, position, reply });
    }

    /// What the server says is wrong with the document.
    pub fn diagnostics(&self, doc: Doc, reply: Reply<Vec<Diagnostic>>) {
        self.send(Job::Diagnostics { doc, reply });
    }

    fn send(&self, job: Job) {
        // The thread only stops once every handle is gone, so a failed send means it panicked. The
        // caller still hears back rather than waiting forever.
        if let Err(mpsc::SendError(job)) = self.jobs.send(job) {
            job.fail(LspError::Closed);
        }
    }
}

/// What the worker's thread owns: the server, and what it knows about it.
struct Session {
    client: LspClient,
    ask: Duration,
    encoding: Encoding,
    /// Whether the server answers `textDocument/diagnostic`.
    pulls: bool,
    documents: HashMap<PathBuf, DocumentSync>,
    /// The newest set the server published for each document.
    published: HashMap<Uri, PublishDiagnosticsParams>,
    /// `None` until the server says; a server that never does is taken as ready.
    quiescent: Option<bool>,
    /// Set once the server has stopped, so later jobs fail at once instead of timing out.
    exited: bool,
}

fn run(mut session: Session, queue: Receiver<Job>) {
    while let Ok(first) = queue.recv() {
        // Take everything already waiting. A newer diagnostics request makes an older one for the
        // same document moot: the text it asked about is gone.
        for job in triage(std::iter::once(first).chain(queue.try_iter()).collect()) {
            session.run(job);
        }
    }
    let _ = session.client.shutdown(Duration::from_secs(2));
}

/// The jobs worth running from a batch, in order. Every diagnostics request but the newest for its
/// document is answered `Superseded` and dropped.
fn triage(batch: Vec<Job>) -> Vec<Job> {
    let newest = |path: &Path| batch.iter().rposition(|job| matches!(job, Job::Diagnostics { doc, .. } if doc.path == path));
    let keep_index: Vec<bool> = batch
        .iter()
        .enumerate()
        .map(|(index, job)| match job {
            Job::Diagnostics { doc, .. } => newest(&doc.path) == Some(index),
            _ => true,
        })
        .collect();
    let mut keep = Vec::with_capacity(batch.len());
    for (job, kept) in batch.into_iter().zip(keep_index) {
        if kept {
            keep.push(job);
        } else {
            job.fail(LspError::Superseded);
        }
    }
    keep
}

impl Session {
    fn run(&mut self, job: Job) {
        self.hear();
        if self.exited {
            return job.fail(LspError::Closed);
        }
        match job {
            Job::Navigate { doc, position, reply } => reply(self.navigate(&doc, position)),
            Job::References { doc, position, reply } => reply(self.references(&doc, position)),
            Job::Hover { doc, position, reply } => reply(self.hover(&doc, position)),
            Job::Diagnostics { doc, reply } => reply(self.diagnostics(&doc)),
        }
    }

    /// Opens the document if the server has not seen it, or tells it about text it has not seen, and
    /// records either only once the send worked.
    fn sync(&mut self, doc: &Doc) -> Result<(), LspError> {
        match self.documents.get_mut(&doc.path) {
            None => {
                let id = language_id(&doc.path).unwrap_or("plaintext");
                self.client.did_open(&doc.path, id, 1, &doc.text)?;
                self.documents.insert(doc.path.clone(), DocumentSync::opened(doc.text.clone()));
            }
            Some(document) => {
                if let Some(version) = document.next(&doc.text) {
                    self.client.did_change(&doc.path, version, &doc.text)?;
                    document.sent(&doc.text, version);
                }
            }
        }
        Ok(())
    }

    fn navigate(&mut self, doc: &Doc, position: Position) -> Result<Navigation, LspError> {
        self.sync(doc)?;
        let (ask, at) = (self.ask, to_server(&doc.text, position, self.encoding));
        let answer = until_settled(ask, || self.client.definition(&doc.path, at, ask))?;
        let targets: Vec<Target> = definition_links(answer)
            .into_iter()
            .map(|link| self.target(doc, link.target_uri, link.target_selection_range))
            .collect();
        if !lands_on_itself(&targets, &path_to_uri(&doc.path)?, position) {
            return Ok(Navigation { found: Found::Definition, targets });
        }
        Ok(Navigation { found: Found::References, targets: self.references(doc, position)? })
    }

    fn references(&mut self, doc: &Doc, position: Position) -> Result<Vec<Target>, LspError> {
        self.sync(doc)?;
        let (ask, at) = (self.ask, to_server(&doc.text, position, self.encoding));
        let answer = until_settled(ask, || self.client.references(&doc.path, at, false, ask))?;
        let mut targets: Vec<Target> =
            answer.unwrap_or_default().into_iter().map(|l| self.target(doc, l.uri, l.range)).collect();
        sort_targets(&mut targets);
        Ok(targets)
    }

    fn hover(&mut self, doc: &Doc, position: Position) -> Result<Option<Hover>, LspError> {
        self.sync(doc)?;
        let (ask, at) = (self.ask, to_server(&doc.text, position, self.encoding));
        let hover = until_settled(ask, || self.client.hover(&doc.path, at, ask))?;
        Ok(hover.map(|mut hover| {
            hover.range = hover.range.map(|range| range_from_server(&doc.text, range, self.encoding));
            hover
        }))
    }

    fn diagnostics(&mut self, doc: &Doc) -> Result<Vec<Diagnostic>, LspError> {
        self.sync(doc)?;
        let ask = self.ask;
        let found = if self.pulls {
            // An empty answer from a server still loading would read as a clean file.
            self.wait_until_quiet();
            until_settled(ask, || self.client.pull_diagnostics(&doc.path, ask))?
        } else {
            self.published_for(doc)?
        };
        Ok(found
            .into_iter()
            .map(|mut diagnostic| {
                diagnostic.range = range_from_server(&doc.text, diagnostic.range, self.encoding);
                diagnostic
            })
            .collect())
    }

    /// The newest set the server published for the document's current text. It waits for the first
    /// such set, then keeps taking newer ones until [`DEFAULT_SETTLE`] passes with none: a server can
    /// publish a quick guess before its real answer.
    fn published_for(&mut self, doc: &Doc) -> Result<Vec<Diagnostic>, LspError> {
        let uri = path_to_uri(&doc.path)?;
        let version = self.documents.get(&doc.path).map_or(1, DocumentSync::version);
        let current = |session: &Session| {
            session.published.get(&uri).filter(|set| set.version.is_none_or(|v| v >= version)).cloned()
        };
        let deadline = Instant::now() + self.ask;
        let mut settle_until = current(self).map(|_| Instant::now() + DEFAULT_SETTLE);
        loop {
            let until = settle_until.unwrap_or(deadline).min(deadline);
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            match self.client.messages.recv_timeout(left) {
                Ok(message) => {
                    let about_us = matches!(&message, ServerMessage::Diagnostics(set) if set.uri == uri);
                    self.note(message);
                    if about_us && current(self).is_some() {
                        settle_until = Some(Instant::now() + DEFAULT_SETTLE);
                    }
                }
                Err(_) => break,
            }
        }
        current(self).map(|set| set.diagnostics).ok_or(LspError::Timeout)
    }

    /// Where an answer points, with its range in characters and its line's text. The document's own
    /// text is used for itself, and a file on disk for any other; a file that cannot be read keeps the
    /// server's columns and no line text.
    fn target(&self, doc: &Doc, uri: Uri, range: Range) -> Target {
        let here = path_to_uri(&doc.path).ok();
        let disk;
        let text = if Some(&uri) == here.as_ref() {
            Some(doc.text.as_str())
        } else {
            disk = uri_to_path(&uri).and_then(|path| std::fs::read_to_string(path).ok());
            disk.as_deref()
        };
        match text {
            Some(text) => Target {
                range: range_from_server(text, range, self.encoding),
                line_text: text.split('\n').nth(range.start.line as usize).unwrap_or("").trim().to_string(),
                uri,
            },
            None => Target { uri, range, line_text: String::new() },
        }
    }

    /// Reads what the server said on its own since the last look.
    fn hear(&mut self) {
        while let Ok(message) = self.client.messages.try_recv() {
            self.note(message);
        }
    }

    fn note(&mut self, message: ServerMessage) {
        match message {
            ServerMessage::Status { quiescent } => self.quiescent = Some(quiescent),
            ServerMessage::Exited => self.exited = true,
            ServerMessage::Diagnostics(set) => {
                let newer = self.published.get(&set.uri).is_none_or(|old| set.version >= old.version);
                if newer {
                    self.published.insert(set.uri.clone(), set);
                }
            }
            ServerMessage::Log(_) => {}
        }
    }

    /// Waits, up to `ask`, for a server that said it is busy to say it is done.
    fn wait_until_quiet(&mut self) {
        let deadline = Instant::now() + self.ask;
        while self.quiescent == Some(false) && !self.exited {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.client.messages.recv_timeout(left) {
                Ok(message) => self.note(message),
                Err(_) => return,
            }
        }
    }
}

/// The file a `file://` URI names. `None` for any other scheme, such as a server's virtual docs.
fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    if uri.scheme().map(|s| s.as_str()) != Some("file") {
        return None;
    }
    let path = uri.path().as_str();
    let decoded = percent_decode(path)?;
    Some(PathBuf::from(decoded))
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
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
    String::from_utf8(out).ok()
}

/// The code a server answers with when the text changed under a request, which LSP 3.17 names
/// `ContentModified`. rust-analyzer sends it for a request that arrives just after a `didChange`.
pub const CONTENT_MODIFIED: i64 = -32801;

/// The code for a request the server dropped and wants asked again, which LSP 3.17 names
/// `ServerCancelled`. A pull for diagnostics can get it while the server is busy.
pub const SERVER_CANCELLED: i64 = -32802;

/// Asks again while the server says the text moved under it, as the spec tells a client to, backing
/// off a little each time. Any other answer, and the last one once `ask` has passed, goes back as is.
pub fn until_settled<T>(ask: Duration, mut request: impl FnMut() -> Result<T, LspError>) -> Result<T, LspError> {
    let deadline = Instant::now() + ask;
    let mut pause = Duration::from_millis(50);
    loop {
        match request() {
            Err(LspError::Server { code: CONTENT_MODIFIED | SERVER_CANCELLED, .. }) if Instant::now() + pause < deadline => {
                thread::sleep(pause);
                pause = (pause * 2).min(Duration::from_millis(800));
            }
            answer => return answer,
        }
    }
}

#[cfg(test)]
mod tests;
