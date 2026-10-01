//! The Pull request story's repository: a small Rust crate written to disk, so rust-analyzer answers
//! hover, definition, uses and names on it, and the pull request's changes over it. Plain data: the
//! files at the head commit, and for each changed file the rows the change removed.

use std::{
    fs,
    path::{Path, PathBuf},
};

use beui::{InlineHunk, RowMap};

/// The files the pull request did not change, at its head. A jump into one opens it Brought In.
const UNCHANGED: &[(&str, &str)] = &[
    (".gitignore", "/target\n"),
    ("Cargo.toml", "[package]\nname = \"relay\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
    (
        "src/lib.rs",
        "//! A small HTTP relay: it forwards a request upstream and streams the answer back.\n\npub mod config;\npub mod request;\npub mod response;\npub mod stream;\n",
    ),
    (
        "src/config.rs",
        "use std::time::Duration;\n\n/// How long a client may keep an aborted stream before the relay drops it.\npub const PATIENCE_MS: u64 = 50;\n\n/// The patience as a duration.\npub fn patience() -> Duration {\n    Duration::from_millis(PATIENCE_MS)\n}\n",
    ),
];

/// One changed file: its text at the head, and each hunk as the head row its added rows start at,
/// the rows it removed there, and how many rows it added.
struct Change {
    path: &'static str,
    head: &'static str,
    hunks: &'static [(usize, &'static [&'static str], usize)],
}

const REQUEST: &str = "use crate::config;
use crate::response::Response;
use crate::stream::ByteStream;

/// One request the relay is answering.
pub struct RequestContext {
    pub response: Option<Response>,
    pub byte_stream: Option<ByteStream>,
}

impl RequestContext {
    /// The client went away while the answer was on its way.
    pub fn on_aborted(&mut self) {
        if let Some(response) = self.response.as_mut() {
            // A stream aborted between chunks still owns the sink, and the top of it
            // wrote a second set of headers onto a socket the server had already taken back.
            if response.flags.has_written_status && self.byte_stream.is_some() {
                if let Some(stream) = self.byte_stream.as_mut() {
                    stream.detach();
                }
            }
            response.flags.aborted = 1;
        }
    }

    /// How long the relay waits on a client that went quiet.
    pub fn patience(&self) -> std::time::Duration {
        config::patience()
    }
}
";

const RESPONSE: &str = "/// What the relay has told the client so far.
#[derive(Default)]
pub struct Flags {
    pub aborted: bool,
    pub aborted_mid_chunk: bool,
    pub has_written_status: bool,
}

/// The answer going back to the client.
#[derive(Default)]
pub struct Response {
    pub status: u16,
    pub flags: Flags,
}

impl Response {
    /// Writes the status line once.
    pub fn write_status(&mut self, status: u16) {
        self.status = status;
        self.flags.has_written_status = true;
    }
}
";

const STREAM: &str = "/// A body the upstream sends in chunks.
#[derive(Default)]
pub struct ByteStream {
    pub sink: Option<Vec<u8>>,
    pub pending: Vec<u8>,
}

impl ByteStream {
    /// Lets go of the sink and anything not yet sent.
    pub fn detach(&mut self) {
        self.sink = None;
        self.pending.clear();
    }
}
";

const ABORT_TEST: &str = "use relay::request::RequestContext;
use relay::response::Response;
use relay::stream::ByteStream;

#[test]
fn an_abort_between_chunks_detaches_the_stream() {
    let mut response = Response::default();
    response.write_status(200);
    let mut request = RequestContext { response: Some(response), byte_stream: Some(ByteStream::default()) };
    request.on_aborted();
    assert!(request.byte_stream.unwrap().sink.is_none());
}
";

const CHANGES: &[Change] = &[
    Change {
        path: "src/request.rs",
        head: REQUEST,
        hunks: &[(
            13,
            &["        if let Some(response) = self.response.as_mut() {", "            response.write_status(200);", "        }"],
            10,
        )],
    },
    Change { path: "src/response.rs", head: RESPONSE, hunks: &[(19, &[], 1)] },
    Change { path: "src/stream.rs", head: STREAM, hunks: &[(10, &["        self.sink = None;"], 2)] },
    Change { path: "tests/abort.rs", head: ABORT_TEST, hunks: &[(0, &[], 12)] },
];

/// A changed file as the review shows it.
pub struct Shown {
    pub path: &'static str,
    /// The head's rows with each hunk's removed rows above its added ones.
    pub text: String,
    pub hunks: Vec<InlineHunk>,
    pub rows: RowMap,
}

impl Shown {
    pub fn added(&self) -> usize {
        self.hunks.iter().map(|h| h.added.len()).sum()
    }

    pub fn removed(&self) -> usize {
        self.hunks.iter().map(|h| h.removed.len()).sum()
    }
}

fn shown(change: &Change) -> Shown {
    let head: Vec<&str> = change.head.trim_end_matches('\n').split('\n').collect();
    let mut rows: Vec<&str> = Vec::new();
    let mut hunks = Vec::new();
    let mut next = 0;
    for (n, &(at, removed, added)) in change.hunks.iter().enumerate() {
        rows.extend(&head[next..at]);
        let removed_rows = rows.len()..rows.len() + removed.len();
        rows.extend(removed);
        let added_rows = rows.len()..rows.len() + added;
        rows.extend(&head[at..at + added]);
        next = at + added;
        hunks.push(InlineHunk::new(format!("{}-{n}", change.path), removed_rows, added_rows));
    }
    rows.extend(&head[next..]);
    let rows_map = RowMap::new(&hunks);
    Shown { path: change.path, text: rows.join("\n"), hunks, rows: rows_map }
}

/// The repository on disk, written fresh, and the pull request's changed files as shown.
pub struct Fixture {
    pub root: PathBuf,
    pub changed: Vec<Shown>,
}

impl Fixture {
    pub fn write() -> Self {
        let root = std::env::temp_dir().join("atelier-gallery-pr");
        let files = UNCHANGED.iter().copied().chain(CHANGES.iter().map(|c| (c.path, c.head)));
        for (path, text) in files {
            let path = root.join(path);
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&path, text);
        }
        let root = atelier_lsp::canonical(&root);
        Self { root, changed: CHANGES.iter().map(shown).collect() }
    }

    /// The changed file at `relative`, by its index.
    pub fn changed_at(&self, relative: &str) -> Option<usize> {
        self.changed.iter().position(|f| f.path == relative)
    }

    /// `path` relative to the repository, when it is in it.
    pub fn relative(&self, path: &Path) -> Option<String> {
        path.strip_prefix(&self.root).ok().map(|p| p.to_string_lossy().into_owned())
    }
}

/// Every file in the repository at `root`, relative and sorted, as Go to file offers them: what
/// `.gitignore` leaves, hidden files left out. It reads the disk, so call it off the UI thread.
pub fn list_files(root: &Path) -> Vec<String> {
    let mut out: Vec<String> = ignore::WalkBuilder::new(root)
        .require_git(false)
        .build()
        .flatten()
        .filter(|entry| entry.file_type().is_some_and(|t| t.is_file()))
        .filter_map(|entry| entry.path().strip_prefix(root).ok().map(|p| p.to_string_lossy().into_owned()))
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests;
