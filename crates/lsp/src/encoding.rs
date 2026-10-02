//! Where a column is, in the unit a server counts.
//!
//! gpui-base counts a column in characters. LSP servers count in UTF-16 units unless the client and
//! the server agree on another encoding. The worker offers UTF-32 (characters) and UTF-16, and
//! converts at its boundary with these functions, so nothing outside it deals with encodings.

mod helpers;
mod types;

pub use helpers::{from_server, range_from_server, to_server};
pub use types::Encoding;

#[cfg(test)]
use lsp_types::{Position, PositionEncodingKind};

#[cfg(test)]
mod tests;
