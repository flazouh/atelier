//! JSON-RPC 2.0, one message a line, as ACP frames it. Both sides send requests: atelier asks the agent to
//! start a session or run a prompt, and the agent asks atelier for a permission. An id is a number or a
//! string; atelier numbers its own and gives the agent's back as it came.

mod helpers;
mod structs;
mod types;

pub(super) use helpers::{error, notification, parse, request, result};
pub(super) use structs::RpcError;
pub(super) use types::{Incoming, METHOD_NOT_FOUND};
