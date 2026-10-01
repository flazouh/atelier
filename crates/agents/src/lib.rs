//! Agents in atelier. [`session`] is atelier's own model of an agent session, and [`claude_code`] is a
//! backend for it. `claude` holds what is particular to Claude for atelier-ui (its mark, its colours,
//! its words), handed over as an [`atelier_ui::AgentLook`]; atelier-ui itself knows no agent.

mod assets;
pub mod claude;
pub mod claude_code;
pub mod coding_agents;
pub mod commands;
pub mod labs;
pub mod own;
pub mod registry;
pub mod session;
pub mod subprocess;

pub use assets::Assets;

#[cfg(test)]
mod testing;
