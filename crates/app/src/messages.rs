//! Messages in the app: one screen for the channels and messages of any messaging provider. It is written once against
//! the `MessagingProvider` trait, as the Tasks screen is against `TasksProvider`, so Slack, Discord and the providers that
//! come later need nothing of their own here. See `docs/capabilities/messaging-v1.md`.
#[cfg(debug_assertions)]
pub mod demo;
pub mod map;
pub mod pane;
pub mod rows;
pub mod source;
