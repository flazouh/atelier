//! The "Merge" story: the merge button in every state, the PR card with its button, and the merge box.
//!
//! The story is the owner the components report to. It keeps the reader's last method for the
//! repository, as the app will, so a method picked on one button is the method on every button after
//! it; it prints each press and shows the last one under the title.

mod helpers;
mod structs;
mod types;

pub use helpers::states;
pub use structs::MergeStory;
