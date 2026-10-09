//! The Discord provider of the `messaging` capability (`docs/capabilities/messaging-v1.md`).
//!
//! It runs `discordcli` (github.com/flazouh/discordcli) for every call, so it needs no login of its own: it uses the
//! Discord session `discordcli` already has. The command runs through a [`Runner`]. [`CommandRunner`] runs the real
//! program; a test gives a fake that behaves like a small Discord, and no test reaches the real service.
//!
//! The login is a user token, which Discord's terms do not allow for automation. So the provider reads by default and
//! sends only when [`DiscordConfig::allow_writes`] is `true` (see `docs/capabilities/discord-notes.md`).
//!
//! What `discordcli` cannot do, [`DiscordMessaging`] does not list: `edit`, `delete`, `react`, `mark_read`, `person`,
//! `export` and `import`. A call for one of them returns `Unsupported` without running anything.
mod core;
mod discord;
mod helpers;
mod poll;
mod structs;
mod text;
mod traits;
mod types;
mod wire;

pub use discord::DiscordMessaging;
pub use helpers::{error_of, is_snowflake, snowflake_ms};
pub use structs::{CommandRunner, DiscordConfig, Output};
pub use text::{from_discord, to_discord};
pub use traits::Runner;
pub use types::RunError;

#[cfg(test)]
mod fake;
#[cfg(test)]
mod tests;
