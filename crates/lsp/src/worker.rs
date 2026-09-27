//! One [`LspClient`] on a thread of its own, so a caller on a UI thread never waits on a server.
//!
//! Every request carries the buffer's whole text. The worker tells the server about text it has not
//! seen before it asks, so an answer is always about what the user sees. Answers come back through a
//! callback on the worker's thread; the caller moves them wherever it needs them.
//!
//! Diagnostics are pulled (LSP 3.17 `textDocument/diagnostic`), not waited for. A published set can
//! be about older text: under load rust-analyzer tags a stale result with the newest version. A
//! pulled one is worked out on the text the server has when it answers.
//!
//! The server stops when the last [`LspWorker`] handle is dropped.

use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use lsp_types::{Diagnostic, GotoDefinitionResponse, Hover, LocationLink, Position};

use crate::{LspClient, LspError, ServerMessage};

/// Where an answer goes. It runs on the worker's thread.
pub type Reply<T> = Box<dyn FnOnce(Result<T, LspError>) + Send>;

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
    pub fn change(&mut self, text: &str) -> Option<i32> {
        if text == self.sent {
            return None;
        }
        self.version += 1;
        self.sent = text.to_string();
        Some(self.version)
    }

    /// The version of the text the server last got.
    pub fn version(&self) -> i32 {
        self.version
    }
}

/// Every answer to `textDocument/definition` as links, the one shape an editor needs. A plain
/// location has no separate name range, so its whole range serves as both.
pub fn definition_links(answer: Option<GotoDefinitionResponse>) -> Vec<LocationLink> {
    let link = |location: lsp_types::Location| LocationLink {
        origin_selection_range: None,
        target_uri: location.uri,
        target_range: location.range,
        target_selection_range: location.range,
    };
    match answer {
        None => vec![],
        Some(GotoDefinitionResponse::Scalar(location)) => vec![link(location)],
        Some(GotoDefinitionResponse::Array(locations)) => locations.into_iter().map(link).collect(),
        Some(GotoDefinitionResponse::Link(links)) => links,
    }
}

/// The first line a definition answer points at, zero-based.
pub fn first_line(links: &[LocationLink]) -> Option<u32> {
    links.first().map(|l| l.target_selection_range.start.line)
}

enum Job {
    Definition { text: String, position: Position, reply: Reply<Option<GotoDefinitionResponse>> },
    Hover { text: String, position: Position, reply: Reply<Option<Hover>> },
    Diagnostics { text: String, reply: Reply<Vec<Diagnostic>> },
}

/// A cheap handle to the thread that owns the server. Clone it for each place that asks.
#[derive(Clone)]
pub struct LspWorker {
    jobs: Sender<Job>,
    path: PathBuf,
}

impl LspWorker {
    /// Moves `client` onto its own thread. `path` is the one open document, which the server already
    /// has as `opened`. `ask` bounds each request.
    pub fn start(client: LspClient, path: PathBuf, opened: DocumentSync, ask: Duration) -> Self {
        let (jobs, queue) = mpsc::channel();
        let thread_path = path.clone();
        thread::spawn(move || run(client, &thread_path, opened, ask, queue));
        Self { jobs, path }
    }

    /// The document this worker serves.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Where the symbol at `position` in `text` is defined.
    pub fn definition(&self, text: String, position: Position, reply: Reply<Option<GotoDefinitionResponse>>) {
        self.send(Job::Definition { text, position, reply });
    }

    /// What the symbol at `position` in `text` is.
    pub fn hover(&self, text: String, position: Position, reply: Reply<Option<Hover>>) {
        self.send(Job::Hover { text, position, reply });
    }

    /// What the server says is wrong with `text`.
    pub fn diagnostics(&self, text: String, reply: Reply<Vec<Diagnostic>>) {
        self.send(Job::Diagnostics { text, reply });
    }

    fn send(&self, job: Job) {
        // The thread only stops once every handle is gone, so a failed send means it panicked. The
        // caller still hears back rather than waiting forever.
        if let Err(mpsc::SendError(job)) = self.jobs.send(job) {
            match job {
                Job::Definition { reply, .. } => reply(Err(LspError::Closed)),
                Job::Hover { reply, .. } => reply(Err(LspError::Closed)),
                Job::Diagnostics { reply, .. } => reply(Err(LspError::Closed)),
            }
        }
    }
}

fn run(mut client: LspClient, path: &Path, mut document: DocumentSync, ask: Duration, queue: Receiver<Job>) {
    // `None` until the server says; a server that never does is taken as ready.
    let mut quiescent: Option<bool> = None;
    for job in queue {
        heard(&client, &mut quiescent);
        match job {
            Job::Definition { text, position, reply } => reply(
                sync(&mut client, path, &mut document, &text)
                    .and_then(|()| until_settled(ask, || client.definition(path, position, ask))),
            ),
            Job::Hover { text, position, reply } => reply(
                sync(&mut client, path, &mut document, &text)
                    .and_then(|()| until_settled(ask, || client.hover(path, position, ask))),
            ),
            Job::Diagnostics { text, reply } => reply(sync(&mut client, path, &mut document, &text).and_then(|()| {
                // An empty answer from a server still loading would read as a clean file.
                wait_until_quiet(&client, &mut quiescent, ask);
                until_settled(ask, || client.pull_diagnostics(path, ask))
            })),
        }
    }
    let _ = client.shutdown(Duration::from_secs(2));
}

/// Reads what the server said on its own since the last look. Only its status matters here:
/// published diagnostics are replaced by pulled ones.
fn heard(client: &LspClient, quiescent: &mut Option<bool>) {
    while let Ok(message) = client.messages.try_recv() {
        if let ServerMessage::Status { quiescent: quiet } = message {
            *quiescent = Some(quiet);
        }
    }
}

/// Waits, up to `ask`, for a server that said it is busy to say it is done.
fn wait_until_quiet(client: &LspClient, quiescent: &mut Option<bool>, ask: Duration) {
    heard(client, quiescent);
    let deadline = std::time::Instant::now() + ask;
    while *quiescent == Some(false) {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match client.messages.recv_timeout(left) {
            Ok(ServerMessage::Status { quiescent: quiet }) => *quiescent = Some(quiet),
            Ok(_) => {}
            Err(_) => return,
        }
    }
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
    let deadline = std::time::Instant::now() + ask;
    let mut pause = Duration::from_millis(50);
    loop {
        match request() {
            Err(LspError::Server { code: CONTENT_MODIFIED | SERVER_CANCELLED, .. })
                if std::time::Instant::now() + pause < deadline =>
            {
                thread::sleep(pause);
                pause = (pause * 2).min(Duration::from_millis(800));
            }
            answer => return answer,
        }
    }
}

fn sync(client: &mut LspClient, path: &Path, document: &mut DocumentSync, text: &str) -> Result<(), LspError> {
    match document.change(text) {
        Some(version) => client.did_change(path, version, text),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests;
