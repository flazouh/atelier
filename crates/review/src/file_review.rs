//! One file's part in a turn: what it was, what it is, and the hunks between.

mod helpers;
mod structs;
mod types;

pub use structs::FileReview;
pub use types::{Change, Content};

#[cfg(test)]
mod tests;
