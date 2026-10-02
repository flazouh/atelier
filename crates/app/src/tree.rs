//! A project's file tree as rows: folders first, then files, each by name without regard to case, as
//! VS Code and Zed list them. Only open folders show their children. Pure, so the whole shape is
//! tested without a window; the view draws [`ProjectTree::rows`] as a virtual list.

mod helpers;
mod structs;

pub use helpers::ancestors;
pub use structs::ProjectTree;
#[cfg(test)]
pub use structs::Row;

#[cfg(test)]
use std::collections::HashSet;
#[cfg(test)]
use atelier_project::Entry;

#[cfg(test)]
mod tests;
