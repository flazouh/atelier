/// Biggest head (request line and headers) the gateway reads.
pub(super) const HEAD_MAX: usize = 16 * 1024;
/// Biggest body. A tool call is a few kilobytes; an agent that sends more is not calling a tool.
pub(super) const BODY_MAX: usize = 1024 * 1024;

/// Why a request could not be read. Each one has an HTTP answer, except a client that went away.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReadError {
    /// The client closed the connection, or stopped talking, before it sent a request.
    Gone,
    Malformed,
    /// The head or the body is over its limit.
    TooLarge,
    /// A body in chunks: the gateway wants a length.
    Chunked,
}
