//! The MCP messages, as JSON-RPC 2.0: what the gateway answers to `initialize`, `ping`, `tools/list` and `tools/call`.
//! It knows nothing of HTTP or of any one capability; the server hands it a parsed message and the tool sets.
mod helpers;
mod types;

pub(crate) use helpers::{handle, supported_version};
pub(crate) use types::Outcome;
