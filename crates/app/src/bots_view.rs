//! The bot library: a lens of the shell with the bots in the sidebar and the one chosen in the main area, read-only.
//! The bots come from `atelier-bots` (a folder next to the settings file, seeded with the starter crew) and
//! `atelier-bot-face` draws them. The outline of a face is the theme's ink, so the bots keep their edge on a dark page.
//! The profile starts a session of its bot: the page says so as a [`BotsEvent`], and the window that shows it opens the session.
//! The builder that makes and edits bots is the next step.

pub(crate) mod consts;
pub(crate) mod helpers;
mod structs;
#[cfg(test)]
mod tests;
mod types;

pub use helpers::{library_root, register};
pub use structs::BotsPage;
pub use types::BotsEvent;
