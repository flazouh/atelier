//! Each agent's look for beui. beui knows no agent; this crate holds what is particular to one (its
//! mark, its colours, its words) and hands it to beui as an [`beui::AgentLook`].

mod assets;
pub mod claude;
pub mod coding_agents;
pub mod labs;

pub use assets::Assets;
