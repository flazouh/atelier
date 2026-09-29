//! Agents in lathe. [`session`] is lathe's own model of an agent session, and [`claude_code`] is a
//! backend for it. `claude` holds what is particular to Claude for beui (its mark, its colours, its
//! words), handed over as an [`beui::AgentLook`]; beui itself knows no agent.

mod assets;
pub mod claude;
pub mod claude_code;
pub mod coding_agents;
pub mod labs;
pub mod registry;
pub mod session;
pub mod subprocess;

pub use assets::Assets;
