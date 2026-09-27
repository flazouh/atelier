//! One [`LspClient`] on a thread of its own, so a caller on a UI thread never waits on a server.
//!
//! Every request carries the buffer's whole text. The worker tells the server about text it has not
//! seen before it asks, so an answer is always about what the user sees. Answers come back through a
//! callback on the worker's thread; the caller moves them wherever it needs them.
//!
//! The server stops when the last [`LspWorker`] handle is dropped.

use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread,
    time::Duration,
};

use lsp_types::{GotoDefinitionResponse, Hover, LocationLink, Position, PublishDiagnosticsParams};

use crate::{DEFAULT_SETTLE, LspClient, LspError, ServerMessage};

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
    Diagnostics { text: String, reply: Reply<PublishDiagnosticsParams> },
}

/// A cheap handle to the thread that owns the server. Clone it for each place that asks.
#[derive(Clone)]
pub struct LspWorker {
    jobs: Sender<Job>,
    path: PathBuf,
}

impl LspWorker {
    /// Moves `client` onto its own thread. `path` is the one open document, which the server already
    /// has as `opened`. `ask` bounds each request and each wait for diagnostics.
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

    /// What the server says is wrong with `text`, once it has settled.
    pub fn diagnostics(&self, text: String, reply: Reply<PublishDiagnosticsParams>) {
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
    // The last settled set, so asking twice about the same text answers at once. A server says
    // nothing new about text it has already checked.
    let mut settled: Option<PublishDiagnosticsParams> = None;
    for job in queue {
        match job {
            Job::Definition { text, position, reply } => reply(
                sync(&mut client, path, &mut document, &text)
                    .and_then(|()| until_settled(ask, || client.definition(path, position, ask))),
            ),
            Job::Hover { text, position, reply } => reply(
                sync(&mut client, path, &mut document, &text)
                    .and_then(|()| until_settled(ask, || client.hover(path, position, ask))),
            ),
            Job::Diagnostics { text, reply } => {
                let answer = sync(&mut client, path, &mut document, &text).and_then(|()| {
                    let version = document.version();
                    let known = settled.clone().filter(|s| s.version == Some(version));
                    let fresh = match known {
                        Some(known) => Ok(newest(&client, known)),
                        None => client.wait_for_diagnostics_at(path, Some(version), ask, DEFAULT_SETTLE),
                    }?;
                    settled = Some(fresh.clone());
                    Ok(fresh)
                });
                reply(answer)
            }
        }
    }
    let _ = client.shutdown(Duration::from_secs(2));
}

/// The code a server answers with when the text changed under a request, which LSP 3.17 names
/// `ContentModified`. rust-analyzer sends it for a request that arrives just after a `didChange`.
pub const CONTENT_MODIFIED: i64 = -32801;

/// Asks again while the server says the text moved under it, as the spec tells a client to, backing
/// off a little each time. Any other answer, and the last one once `ask` has passed, goes back as is.
pub fn until_settled<T>(ask: Duration, mut request: impl FnMut() -> Result<T, LspError>) -> Result<T, LspError> {
    let deadline = std::time::Instant::now() + ask;
    let mut pause = Duration::from_millis(50);
    loop {
        match request() {
            Err(LspError::Server { code: CONTENT_MODIFIED, .. }) if std::time::Instant::now() + pause < deadline => {
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

/// `known`, or a later set for the same file and version the server has sent since.
fn newest(client: &LspClient, known: PublishDiagnosticsParams) -> PublishDiagnosticsParams {
    let mut best = known;
    loop {
        match client.messages.try_recv() {
            Ok(ServerMessage::Diagnostics(params)) if params.uri == best.uri && params.version >= best.version => {
                best = params
            }
            Ok(_) => continue,
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return best,
        }
    }
}

#[cfg(test)]
mod tests;
