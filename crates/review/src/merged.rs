//! One file under review, in the form the inline review edits: both versions in one text, each changed
//! hunk written as its old rows and then its new rows. Everything here is pure text in, text out.

mod helpers;
mod structs;
mod types;

pub use structs::{Anchor, Merged};
pub use types::Side;

#[cfg(test)]
mod tests;
