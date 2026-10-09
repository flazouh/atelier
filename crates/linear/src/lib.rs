//! The Linear provider of the `tasks` capability. [`LinearTasks`] talks to Linear's GraphQL API and answers the
//! [`TasksProvider`](atelier_capabilities::tasks::TasksProvider) calls, so the one tasks screen and the agent tools work
//! on a Linear workspace as they do on Atelier's own tasks. The spec is `docs/capabilities/tasks-v1.md`, section 4.2.
//!
//! The caller hands over the API key. This crate never reads a keychain, and never prints or logs a key.
mod client;
mod provider;

pub use provider::{Config, LinearTasks};
