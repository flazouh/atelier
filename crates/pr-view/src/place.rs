//! Where the threads of a pull request sit in one file's diff. A thread on the new side hangs under the
//! row that holds its line in the head; one on the old side under the row that holds its line in the base.
//! A thread on the whole file, or one whose code has changed since (outdated: it has no line now), has no
//! row, and the view lists them above the code instead.

mod helpers;
mod structs;

pub use helpers::place;
pub use structs::Placement;
