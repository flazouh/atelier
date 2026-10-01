//! A project's pull requests, of its own repository only: alex-31's `PrHub` (`crates/pr-view`, `docs/pr-view.md`), in the right pane
//! in place of the editor while it shows. It opens from the project's ⋯ menu ("Pull requests") or
//! ⌘⇧P, and from a PR chip in an agent's text. Read-only until Alex approves a scratch repository:
//! nothing it does is sent to GitHub. Its services (a small database and caches) open off the UI
//! thread, and the reader's login comes from the host's `gh`.

mod helpers;
mod structs;

pub use helpers::{chips_of, login, open_services};
pub use structs::Pulls;

#[cfg(test)]
use atelier_ui::PrChipData;

#[cfg(test)]
mod tests;
