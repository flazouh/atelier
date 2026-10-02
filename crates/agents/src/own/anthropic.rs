//! Anthropic's Messages API, streaming. Tool use, extended thinking (adaptive) and prompt caching.
//!
//! The request marks three cache breakpoints, so a long session pays for each part once: the last tool
//! definition, the system prompt, and the last block of the conversation. The reply is read event by
//! event ([`StreamState`]), which is pure and does no I/O.

mod helpers;
mod structs;
mod types;

pub use helpers::{block_json, messages_json, request_body};
#[cfg(test)]
pub(crate) use helpers::wants_budget;
pub use structs::{Anthropic, StreamState};
pub use types::DEFAULT_BASE;
