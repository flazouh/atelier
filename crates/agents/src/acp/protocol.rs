//! One conversation with an ACP agent, as pure state: each line the agent writes and each command atelier
//! gives goes in, and the events for the app and the lines to write come out. It touches no process and
//! reads no clock, so the whole protocol is tested without an agent.
//!
//! The handshake is `initialize`, then `session/new`, `session/load` or `session/list` for the goal. When
//! the agent answers that it needs a sign-in, atelier calls `authenticate` with the first method the agent
//! offered and asks again, once. A session is ready when the agent answers with its id: `Started` goes
//! out, the model and mode the request asked for are set, and once the agent has answered those, the
//! messages sent before then are prompted one after another.
//!
//! Cursor puts a model's settings in brackets after its id (`composer-2.5[fast=true]`) and takes only the
//! whole id. atelier names a model by the part before the brackets, and sends the whole id the agent listed.

mod helpers;
mod structs;
mod types;

pub(super) use structs::{Protocol, Step};
pub(super) use types::{Found, Goal};
