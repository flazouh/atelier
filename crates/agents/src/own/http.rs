//! A small HTTP/1.1 client for streaming replies: one request per connection (`Connection: close`),
//! plain or TLS, with the response body read in pieces as it arrives.
//!
//! It is here, not a library, for one reason: a reply can stall for a long time (a model thinks), and the
//! caller must be able to stop it at once. Every wait in this file wakes ten times a second to look at a
//! [`Cancel`](crate::own::message::Cancel) flag, and all parsing works on our own buffer, so a wake never leaves a half-read line.
//! No header of a request appears in an error: a key in `x-api-key` never reaches a log.

mod helpers;
mod structs;
mod traits;
mod types;

pub use helpers::{map_read_error, send};
pub use structs::{Body, HttpOptions, HttpRequest, HttpResponse};
pub use types::HttpError;
