//! Inside a changed row: which words changed. The line diff says a row was replaced; this says which
//! part of it, so the review can highlight the words rather than the whole row.

mod helpers;
mod structs;
mod types;

pub use helpers::{pair_rows, word_changes};
pub use structs::RowChange;

#[cfg(test)]
mod tests;
