//! What the file tree offers beyond opening a file: a menu on a right press, a new file or folder and a rename typed
//! in the tree itself, a copy, a delete that asks first, the path on the clipboard, and a mention of the file in a
//! session. The disk changes go through [`Project::apply`](atelier_project::Project::apply), so they work over SSH too.
mod helpers;
mod impls;
mod structs;
mod types;

#[cfg(test)]
use helpers::{bad_name, copy_name, joined, moved};
pub use structs::{TreeEdit, TreeMenu};
pub use types::TreeEditKind;

#[cfg(test)]
mod tests;
