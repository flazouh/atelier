use std::time::Duration;

/// How often a wait looks at the cancel flag.
pub(super) const WAKE: Duration = Duration::from_millis(100);

pub(super) const MAX_HEAD: usize = 64 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum HttpError {
    Cancelled,
    /// The request could not be made: a bad URL, no route, a refused connection, a TLS failure.
    Connect(String),
    /// The connection broke or went silent.
    Io(String),
    /// The server's answer is not HTTP.
    Protocol(String),
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("stopped"),
            Self::Connect(why) | Self::Io(why) | Self::Protocol(why) => f.write_str(why),
        }
    }
}

pub(super) enum Mode {
    Length(u64),
    /// Bytes left in the current chunk, or `None` between chunks.
    Chunked(Option<u64>),
    UntilClose,
    Done,
}
