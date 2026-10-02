use super::structs::Changed;

pub(super) const CONFIG: &str = "use std::time::Duration;\n\n/// How long a client may keep an aborted stream before the relay drops it.\npub const PATIENCE_MS: u64 = 50;\n\n/// The patience as a duration.\npub fn patience() -> Duration {\n    Duration::from_millis(PATIENCE_MS)\n}\n";

pub(super) const LIB: &str = "//! A small HTTP relay: it forwards a request upstream and streams the answer back.\n\npub mod config;\npub mod request;\npub mod response;\npub mod stream;\n";

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

pub(super) const CHANGED: &[Changed] = &[
    Changed {
        path: "src/request.rs",
        head: REQUEST,
        hunks: &[(13, &["        if let Some(response) = self.response.as_mut() {", "            response.write_status(200);", "        }"], 10)],
    },
    Changed { path: "src/response.rs", head: RESPONSE, hunks: &[(19, &[], 1)] },
    Changed { path: "src/stream.rs", head: STREAM, hunks: &[(10, &["        self.sink = None;"], 2)] },
    Changed { path: "tests/abort.rs", head: ABORT_TEST, hunks: &[(0, &[], 12)] },
];

pub const NUMBER: u64 = 3344;

pub(super) const COMMITS: [&str; 6] = ["Detach the byte stream before a second write", "Test an abort between chunks", "Hold the sink until flush", "Name the fields", "Drop the extra render", "Keep the status flag"];

pub const BODY: &str = "A client that aborted between two chunks left the relay writing into a closed sink. The stream now detaches on abort, and a second write does nothing.";

pub(super) const LOG: &str = "2026-09-29T04:00:00.0000001Z Current runner version: '2.337.0'
2026-09-29T04:00:01.0000001Z Complete job name: linux-x64
2026-09-29T04:00:02.0000001Z ##[group]Run actions/checkout@v4
2026-09-29T04:00:02.0000002Z with: fetch-depth: 1
2026-09-29T04:00:03.0000001Z ##[group]Run cargo test
2026-09-29T04:00:04.0000001Z running 3 tests
2026-09-29T04:00:05.0000001Z error[E0308]: mismatched types
2026-09-29T04:00:05.0000002Z   --> src/request.rs:22:37: expected `bool`, found integer
2026-09-29T04:00:06.0000001Z ##[error]Process completed with exit code 101.
2026-09-29T04:00:07.0000001Z Post job cleanup.
2026-09-29T04:00:08.0000001Z Cleaning up orphan processes
";

pub(super) const SLOW_LOG: &str = "2026-09-29T04:00:00.0000001Z Current runner version: '2.337.0'
2026-09-29T04:00:03.0000001Z ##[group]Run cargo test
2026-09-29T04:00:05.0000001Z Timed out after 50ms waiting for the socket
2026-09-29T04:00:06.0000001Z ##[error]Process completed with exit code 1.
2026-09-29T04:00:08.0000001Z Cleaning up orphan processes
";
