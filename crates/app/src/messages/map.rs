//! Messages between the capability's types and what the screen draws. The capability knows no UI type and atelier-ui
//! names no provider, so this is the one place that knows both. Pure: nothing here reads a clock or a provider.
mod helpers;
mod structs;
mod types;

#[cfg(test)]
use helpers::{letter_of, safe_markdown, size_words};
pub use helpers::{line_of, row_of};
pub use structs::{Chip, File, Line, Row};
pub use types::{Body, Group};

#[cfg(test)]
mod tests;
