//! Agents in atelier. [`session`] is atelier's own model of an agent session; [`claude_code`] and [`acp`] are
//! backends for it. `claude` and `cursor` hold what is particular to each agent for atelier-ui (its mark, its
//! colours, its words), handed over as an [`atelier_ui::AgentLook`]; atelier-ui itself knows no agent.

pub mod acp;
mod assets;
pub mod claude;
pub mod claude_code;
pub mod coding_agents;
pub mod commands;
pub mod cursor;
pub mod handoff;
pub mod labs;
pub mod own;
mod partial_json;
pub mod registry;
pub mod session;
pub mod subprocess;

pub use assets::Assets;

#[cfg(test)]
mod testing;
