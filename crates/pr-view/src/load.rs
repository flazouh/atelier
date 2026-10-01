//! Reading a pull request from the forge. The reads are independent, so they run at once, each on a
//! thread of its own; every part is sent the moment it arrives. This blocks until all are done, so call
//! it from a background task.

mod helpers;
mod types;

pub use helpers::{load_all, load_parts};
pub use types::LIVE;
