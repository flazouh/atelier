//! The Tasks pane: a project's tasks as a list or a board, one task in full, and the create dialog. It takes
//! the right pane, as the pull requests do. It reads and writes the tasks of the shown provider (`TasksProvider`)
//! off the UI thread and keeps its own copy of the tasks, which the four parts share: a change in one shows
//! in the others. What the provider cannot do has no control, and each way a call fails has its own state.
mod helpers;
mod structs;
mod types;
pub use structs::TasksPane;
pub use types::{Scope, TasksEvent};
#[cfg(test)]
pub use types::{BOARD_LEAST, Mode};
#[cfg(test)]
use helpers::{apply, react};
#[cfg(test)]
use types::{Load, Problem, Reaction, Source};
#[cfg(test)]
use atelier_ui::task_edit::Change;
#[cfg(test)]
mod tests;
