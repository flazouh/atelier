//! Which rows of a session's list changed after a frame's events, so the virtual list measures only
//! those again and keeps its scroll. Each item has a small fingerprint of what its row draws; two lists
//! of fingerprints give the ranges to replace. Pure.

mod helpers;
mod types;

pub use helpers::{activity_fingerprint, arrivals, changes, changes_fingerprint, fingerprint, grouped, rows, waiting_fingerprint, waits};
pub use types::{Arrival, Row};

#[cfg(test)]
use atelier_agents::session::Item;

#[cfg(test)]
mod tests;
