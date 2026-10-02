//! The context budget. A long session fills the model's window with old tool output, which the model
//! rarely needs again. When the conversation grows past the budget, the oldest tool results (and the big
//! strings in old tool inputs, such as a file that was written) are shortened to a note and their first
//! lines, oldest first, until it fits with room to spare. The messages and the order of calls stay, so the
//! conversation still reads. A shortened message changes the prompt cache from that point, so this runs
//! only when the budget is passed, and shortens well below it so it does not run every turn.

mod helpers;
mod structs;
mod types;

pub use helpers::{compact, estimate};
pub use structs::{Budget, Compaction};
