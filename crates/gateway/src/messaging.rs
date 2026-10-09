//! The `messaging` tools: `messaging_channels`, `messaging_history`, `messaging_thread`, `messaging_search` and
//! `messaging_send`, over the providers in a [`Registry`](atelier_capabilities::Registry). The spec is section 8 of
//! `docs/capabilities/messaging-v1.md`.
//!
//! A reply is `messaging_send` with `in_thread_of`, so there is no tool of its own for it. A tool is listed only while a
//! provider that can do it is connected.
mod helpers;
mod render;
mod schemas;
mod structs;

pub use structs::MessagingTools;
