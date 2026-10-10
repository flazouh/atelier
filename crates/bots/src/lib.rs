//! The bots of `docs/bots/bots-v1.md`. A `Bot` is a persona on top of an agent harness: a face, a role, skills,
//! tools, a voice and a version. Its memory is kept as notes in three layers. A `Playbook` is an ordered list of
//! bots. `BotStore` keeps all of it, and `DiskBotStore` keeps it in a folder. `starter_crew` is the ten bots that
//! ship. Nothing here draws: the face is a choice, and `atelier-bot-face` draws it.

mod consts;
mod enums;
mod impls;
mod structs;
#[cfg(test)]
mod tests;
mod traits;

pub use enums::{Access, Body, BotsError, Colour, Harness, MemoryScope, Tool, Voice};
pub use impls::{seed_starters, starter_crew, starter_playbooks};
pub use structs::{
    Bot, BotId, DiskBotStore, FaceChoice, MemoryNote, Playbook, PlaybookStep, Provider, ToolGrant,
};
pub use traits::BotStore;
