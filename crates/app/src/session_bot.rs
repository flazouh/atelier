//! The bot of a session. A session may belong to a bot (`docs/bots/bots-v1.md`): the agent that runs it is the bot's
//! harness and is told the bot's persona, and the session's rows show the bot's face in the mood of what the session
//! does (`docs/bots/faces-v1.md`). A session with no bot has none of this and looks as it always did.
//!
//! `mood_of` says the mood, `agent_of` the agent of a harness, `bots_folder` where the bots are kept, `kept_bot` reads
//! a bot of that folder by its id, and `SessionBot` is what a session keeps: the bot's record and what moves its face.

mod consts;
mod helpers;
mod structs;
#[cfg(test)]
mod tests;

pub use helpers::{agent_of, bots_folder, header_face, kept_bot, mood_of, row_face, seeded_bot, tool_runs};
pub use structs::{BotsFolder, SessionBot};
