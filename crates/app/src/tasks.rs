//! Tasks in the app: the tracker of a project, what atelier-ui shows of it, and the pane that holds them.

mod helpers;
pub mod map;
pub mod pane;
pub mod signal;
mod structs;

pub use helpers::rules;
pub use structs::{Slot, TaskRef};

#[cfg(test)]
mod tests;
