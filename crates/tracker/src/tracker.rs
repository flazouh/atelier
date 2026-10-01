//! The interface to a tracker. Every call blocks and may be slow, so none is made on the UI thread.

mod structs;
mod traits;
mod types;

pub use structs::{StopFlag, Subscription};
pub use traits::Tracker;
pub use types::{TrackerError, TrackerResult};
