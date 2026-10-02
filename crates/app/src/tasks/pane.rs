//! The Tasks pane: a project's tasks as a list or a board, one task in full, and the create dialog. It takes
//! the right pane, as the pull requests do. It reads and writes the project's tracker off the UI thread
//! and keeps its own copy of the tasks, which the four parts share: a change in one shows in the others.

mod structs;
mod types;

pub use structs::TasksPane;
pub use types::TasksEvent;
#[cfg(test)]
pub use types::{BOARD_LEAST, Mode};

#[cfg(test)]
use types::{Load, Source};

#[cfg(test)]
use atelier_ui::task_edit::Change;
#[cfg(test)]
use atelier_tracker::TaskId;

#[cfg(test)]
mod tests;
