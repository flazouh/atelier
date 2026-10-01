//! What the right pane shows: the pull requests, the tasks, or the editor. The reader's last ask
//! wins while it is there, so Cmd+Shift+P over the tasks brings the pull requests up, and hiding
//! them brings the tasks back. The review is not here: it takes the place of the panels.

mod helpers;
mod types;

pub use helpers::front;
pub use types::Front;

#[cfg(test)]
mod tests;
