//! The host's tracker, over the connection: each call of the `Tracker` trait is one request. The store is
//! the host's own file, so a change made on another machine reaches `subscribe` only by a poll of `list`,
//! every [`POLL`], while a receiver is kept.

mod helpers;
mod structs;
mod types;

pub use structs::RemoteTracker;
pub use types::POLL;

#[cfg(test)]
use helpers::changes;

#[cfg(test)]
mod tests;
