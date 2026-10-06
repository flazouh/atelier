//! A tool call's input read while it still streams in: models send it as pieces of JSON, and a backend that shows an
//! edit as it is written needs the text fields of what has come so far. Shared by every backend that gets the pieces.

mod helpers;

pub(crate) use helpers::{closed, fields, string_end, value};

#[cfg(test)]
mod tests;
