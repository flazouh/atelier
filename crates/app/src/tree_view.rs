//! The project's file tree in the sidebar: a virtual list, so a tree of ten thousand files lays out
//! only the rows on screen. A folder opens and closes on a press; a file opens in a tab.

mod helpers;
mod types;

pub use helpers::tree_view;
