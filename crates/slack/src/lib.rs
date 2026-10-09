//! The Slack provider of the `messaging` capability (`docs/capabilities/messaging-v1.md`).
//!
//! It runs `slackcli` (github.com/flazouh/slackcli) for every call, so it needs no login of its own: it uses the Slack
//! session `slackcli` already has. The command runs through a [`Runner`]. [`CommandRunner`] runs the real program; a test
//! gives a fake that behaves like a small Slack, and no test reaches a real workspace.
//!
//! What `slackcli` cannot do, [`SlackMessaging`] does not list: `delete`, `react`, `mark_read`, `export`, `import`. A call
//! for one of them returns `Unsupported` without running anything.

mod helpers;
mod slack;
mod structs;
mod traits;
mod types;
mod wire;

pub use helpers::{Permalink, parse_permalink, to_mrkdwn};
pub use slack::SlackMessaging;
pub use structs::{ChannelSpec, CommandRunner, Output, SlackConfig};
pub use traits::Runner;
pub use types::RunError;

#[cfg(test)]
mod fake;
#[cfg(test)]
mod tests;
