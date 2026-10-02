//! The coding agents atelier can drive, and their marks, as acepe keeps them: Claude Code, Codex, Cursor,
//! Grok, opencode, and Custom, which takes atelier-ui's monogram.

mod helpers;
mod types;

pub(crate) use helpers::bytes;
pub use types::CodingAgent;

#[cfg(test)]
mod tests;
