//! A session's review, as one piece of state: the turns it tracked, where each turn's card sits, what the
//! reader decided and marked, and the comments waiting for the next message or already sent. It is kept
//! in the project's data folder with the session (`review/<session id>.json`), so a session resumed after
//! a restart opens its review as it was left.
//!
//! atelier-review's types are not serializable, so the record holds what rebuilds them: each file's texts
//! before and after its turn, each decided file's baseline and current text (`Merged::diff` gives the
//! hunks still to decide), each mark with the file's version (a mark on a file that changed since stays
//! expired), and each comment's anchor.

mod helpers;
mod structs;
mod types;

pub use helpers::record_path;
pub use structs::{Record, ReviewState};
pub use types::{Approval, Decided};

#[cfg(test)]
use atelier_review::{Content, Merged, TurnReview};
#[cfg(test)]
use crate::review_pane::Scope;

#[cfg(test)]
mod tests;
