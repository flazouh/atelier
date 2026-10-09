//! Mail between the capability's types and what the screen draws. The capability knows no UI type and atelier-ui names no
//! provider, so this is the one place that knows both. Pure: nothing here reads a clock or a provider. Everything a sender
//! wrote comes out as plain text here: a control character or a bidirectional override is taken out, so the screen can draw it
//! as it is.
mod helpers;
mod structs;
mod types;

pub use helpers::{box_rows_of, message_view_of, thread_row_of};
#[cfg(test)]
use helpers::{clean_body, clean_line, cut_at, date_words, links_of, size_words, who_of};
pub use structs::{BoxRow, MessageView, ThreadRow};
#[cfg(test)]
pub use types::{BODY_LIMIT, LINKS_MOST};

#[cfg(test)]
mod tests;
