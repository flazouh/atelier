//! The Review story: the review bar, the changed file tree, and the real editor with the inline review's
//! hunks, one open comment thread and one comment being written, over four fixture files.
//!
//! Pressing a file in the tree, or the file keys, loads that file's text and hunks; each file keeps its
//! own edits while another is open. Accept file accepts every hunk of the open file and moves on. A file
//! with no hunks left counts as reviewed. Put all back restores every file.

mod helpers;
mod structs;
mod types;

pub use helpers::element;
pub use structs::ReviewStory;
