//! Merging: the repository's rules, what the reader may do, and the request and its outcome.

mod structs;
mod types;

pub use structs::{MergeRequest, MergeSettings, QueuePlace};
pub use types::{MergeMethod, MergeOutcome, Rights, UpdateMethod};
