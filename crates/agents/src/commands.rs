//! The commands a composer offers after `/`, from three sources, as neutral data: the agent's own (its
//! init lists them), atelier's own, and the project's skills (`.claude/skills/*/SKILL.md`) and command files
//! (`.claude/commands/*.md`). A command atelier runs itself is atelier's even when the agent lists it too:
//! the agent runs headless and cannot run `/login`, for one.

mod helpers;
mod structs;
mod types;

pub use helpers::{atelier_commands, command_file_from, merge, skill_from};
pub use structs::CommandInfo;
pub use types::CommandSource;

#[cfg(test)]
mod tests;
