//! Where a session can be handed off to: each agent this build offers, and for an agent that runs on providers
//! each provider the reader can use (its signed-in accounts, and OpenRouter when a key is kept).
//!
//! This is the one place that knows the list. The sidebar's row menu and the limit notice both show
//! [`Targets::branches`], and a choice in either comes back as a [`Target`] by its id.

mod consts;
mod impls;
mod structs;
#[cfg(test)]
mod tests;

pub use structs::{Target, Targets};
#[cfg(test)]
pub use structs::AgentChoices;
