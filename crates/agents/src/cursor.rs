//! Cursor: its agent CLI spoken to over ACP (`agent acp`), and its look for atelier-ui. What `agent` sends was
//! checked live; `docs/agents.md` says what it offers and how atelier's modes map to its own.

mod helpers;
mod types;

pub use helpers::{agent, look};
pub use types::BACKEND;

#[cfg(test)]
use types::CUBE;

#[cfg(test)]
use crate::session::PermissionMode;

#[cfg(test)]
mod tests;
