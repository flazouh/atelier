//! The pull requests list on the screen: the reader's working set as Courts, drawn from the disk at once,
//! read again from the forge in the background, and kept current with a backoff. It opens nothing itself:
//! a row reports [`ListEvent::Open`] and the owner decides where the pull request shows.

mod helpers;
mod structs;
mod types;

pub use structs::PullList;
pub use types::ListEvent;
