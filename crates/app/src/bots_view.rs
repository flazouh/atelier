//! The bot library: a plugin whose view has the bots in the sidebar and the one chosen in the main area, read-only.
//! The bots come from `atelier-bots` (a folder next to the settings file, seeded with the starter crew) and
//! `atelier-bot-face` draws them. The outline of a face is the theme's ink, so the bots keep their edge on a dark page.
//! The builder that makes and edits bots is the next step.

pub(crate) mod consts;
pub(crate) mod helpers;
mod impls;
mod structs;
#[cfg(test)]
mod tests;

pub use structs::{BotsPage, BotsPlugin};
