//! A text as rows, and the rows as a token source for the diff.

mod helpers;
mod structs;

pub(crate) use helpers::join;
pub(crate) use structs::{RowTokens, Rows};

#[cfg(test)]
mod tests;
