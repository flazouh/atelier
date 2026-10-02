//! What people said on a pull request: remarks about the whole, threads on lines, and the review that
//! carries a verdict.

mod structs;
mod types;

pub use structs::{Comment, HeldComment, NewLine, Thread, ThreadId};
pub use types::{Author, Remark, Side, Verdict};
