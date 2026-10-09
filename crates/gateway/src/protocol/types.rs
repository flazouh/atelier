use serde_json::Value;

/// The revisions of the spec the gateway speaks, newest first. The newest is the one it offers.
pub(super) const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub(super) const PARSE_ERROR: i64 = -32700;
pub(super) const INVALID_REQUEST: i64 = -32600;
pub(super) const METHOD_NOT_FOUND: i64 = -32601;
pub(super) const INVALID_PARAMS: i64 = -32602;

/// What the server does with a message.
#[derive(Debug, PartialEq)]
pub(crate) enum Outcome {
    /// An answer to send with status 200.
    Reply(Value),
    /// A notice or a reply from the client: nothing to say back, status 202.
    Accepted,
    /// A message that cannot be served at all: the answer goes with status 400.
    Refused(Value),
}
