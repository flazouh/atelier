//! An ACP agent's session updates to atelier's events. The mapper keeps what one session needs between
//! updates: the block that streams, the tool calls it has seen, and the user text a history replays in
//! chunks. It reads no clock and touches no process: the caller passes the time with each update.

mod helpers;
mod structs;
mod types;

pub(super) use structs::Mapper;
