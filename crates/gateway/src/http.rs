//! Just enough HTTP/1.1 for one JSON POST at a time over a loopback socket: read a request, write an answer, close.
//! The gateway keeps no connection open, so there is no keep-alive, no chunking and no pipelining to get wrong.
mod helpers;
mod structs;
mod types;

pub(crate) use helpers::{read_request, write_response};
pub(crate) use structs::{Request, Response};
pub(crate) use types::ReadError;

#[cfg(test)]
mod tests;
