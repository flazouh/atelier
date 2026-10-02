//! What a file of the review keeps for a commit: its text before the turn with the hunks the reader
//! accepted and the edits they made in them (`Merged::baseline`). Undecided and rejected hunks stay out.
//! Pure.

mod helpers;
mod structs;

pub use helpers::{against, kept, own_edits};
pub use structs::Kept;

#[cfg(test)]
mod tests;
