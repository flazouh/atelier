//! Codex: OpenAI's coding agent, spoken to over ACP through the adapter `@agentclientprotocol/codex-acp`, which
//! starts Codex's own app server. (The older `@zed-industries/codex-acp` is archived and cannot read a config the
//! current Codex writes.) What the adapter offers was checked live; `docs/agents.md` says how atelier's modes and
//! models map to its own. This module holds what is particular to Codex: how to start it, and its look for atelier-ui.

mod consts;
mod impls;
mod structs;

pub use consts::BACKEND;
pub use structs::Codex;

#[cfg(test)]
mod tests;
