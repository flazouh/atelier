//! How much of the agent's tool calls a session shows: the one setting behind the Settings page's "Tool calls" choice.
//! It lives in a global, so every open session follows a change at once.

mod helpers;
mod impls;
mod types;

pub use helpers::tool_density;
pub use types::ToolDensity;

#[cfg(test)]
mod tests;
