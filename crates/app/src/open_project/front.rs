//! What the right pane shows: the review, the pull requests, or the editor. The reader's last ask
//! wins while it is there, so Cmd+Shift+P during a review brings the pull requests up, and hiding
//! them brings the review back.

mod helpers;
mod types;

pub use helpers::front;
pub use types::Front;

#[cfg(test)]
mod tests;
