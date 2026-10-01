//! One changed file, ready to draw: both sides in one text (each changed hunk as its old rows then its
//! new rows, the form the inline review edits), and the maps that put a thread or a language-server answer
//! on the right row. The diff itself is `atelier_review::Merged`; this only feeds it the two texts git gave.

mod structs;
mod types;

pub use structs::{FileView, LineMap, Shown};
pub use types::Content;
