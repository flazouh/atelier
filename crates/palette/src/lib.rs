//! The colours the app draws marks in: a subagent's mark, a permission mode's icon, a reply badge's icon. They are named
//! here, once. Nothing else writes one of these hex numbers; a mark takes a [`Hue`] and asks it for its colour.
mod helpers;
mod types;
pub use helpers::{hue_at, kind_color};
pub use types::Hue;
#[cfg(test)]
mod tests;
