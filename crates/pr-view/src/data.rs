//! One pull request as the reader has it so far. The forge answers in parts, so a screen can draw the
//! header while the files are still coming, and a part that fails does not take the others with it.

mod structs;
mod types;

pub use structs::PullData;
pub use types::{Part, PartKind};
