//! The pull request form, in the Ship strip after a push: a title and a body the agent drafts and the
//! reader edits, the base branch, and whether it opens as a draft. Open (⌘↵) asks the forge off the UI
//! thread; a refusal keeps the form and says why.

mod helpers;
mod structs;
mod types;

pub use helpers::bind_keys;
pub use structs::PullForm;
pub use types::FormEvent;
#[cfg(test)]
pub use types::FormStage;

#[cfg(test)]
use gpui_kit::{Entity, IntoElement, Window};

#[cfg(test)]
mod tests;
