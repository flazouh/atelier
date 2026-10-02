//! atelier's own agent: an agent loop that runs inside atelier and calls a model API directly. No child
//! process and no wire format of ours; it is a [`Backend`] like the others, and the UI cannot tell.
//!
//! - [`Model`] is the one seam to a model API: a streaming chat call with tools. [`Anthropic`] is the
//!   Messages API; [`OpenAiCompatible`] is Chat Completions, which covers OpenAI, OpenRouter and the many
//!   servers that copy it.
//! - The tools (`read`, `list`, `search`, `edit`, `write`, `shell`) touch the project only through
//!   `Project`, so the agent works on a local folder and on an SSH host with no other code.
//! - Permissions are atelier's own ([`permission`]). The record of a session is kept in the project
//!   ([`store`]), so a session resumes with no service behind it.
//!
//! Keys come from the caller: [`OwnAgent::from_env`] reads the environment, and a settings screen can
//! hand a [`Secret`] to the constructors. A key is never logged, stored by this module, or put in an event.

pub mod anthropic;
pub mod context;
mod helpers;
pub mod http;
pub mod message;
pub mod openai;
pub mod permission;
mod runner;
pub mod sse;
pub mod store;
mod structs;
pub mod tools;

pub use anthropic::Anthropic;
pub use context::Budget;
pub use message::{Block, Cancel, Delta, Message, Model, ModelError, ModelRequest, Reply, Role, Secret, StopReason, Thinking, TokenUsage, ToolDef};
pub use openai::OpenAiCompatible;

pub use helpers::{anthropic_models, system_prompt};
pub use structs::{OwnAgent, OwnOptions, RetryPolicy};

#[cfg(test)]
mod tests;
