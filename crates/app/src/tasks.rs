//! Tasks in the app: the providers of a project, what atelier-ui shows of them, and the pane that holds them. The
//! local tracker stays only for the rules and the session and pull request links.

mod helpers;
pub mod map;
pub mod pane;
pub mod signal;
pub mod source;
mod structs;

pub use helpers::rules;
pub use structs::{Slot, TaskRef};

#[cfg(test)]
mod tests;
