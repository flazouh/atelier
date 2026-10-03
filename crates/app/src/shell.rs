//! The window: a title bar that is part of the page, the sidebar (the projects in this window, the
//! active project's sessions and its file tree), the agent panel in the middle, the editor on the
//! right, and the status line at the foot. With no project open, the start screen fills the window.
//!
//! Keys: ⌘O opens a folder, ⌘S saves, ⌘W closes the tab, and from GitQuiet's table, `t` goes to a
//! file (while nothing is being typed) and ⌘B and ⌘⇧B hide and show the left and the right pane.

mod fit;
mod helpers;
mod impls;
mod lens;
mod rail;
mod restore;
mod structs;
mod types;
mod view;

pub use view::ShellView;

pub use helpers::bind_keys;
pub use structs::{Quit, Shell};
#[cfg(test)]
pub use structs::{NewSession, OpenSettings};

#[cfg(test)]
use types::WHAT_ATELIER_IS;

#[cfg(test)]
use gpui_kit::{Context, Entity, Window};

#[cfg(test)]
mod tests;
