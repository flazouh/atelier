//! The labs that make models, and their marks, as acepe keeps them (`provider-brand-icons.ts`,
//! `upstream-provider-mark.svelte`): Anthropic, OpenAI, xAI, OpenRouter, GitHub Copilot, and Custom. acepe
//! has no mark for xAI, so it and Custom take atelier-ui's monogram.

mod helpers;
mod types;

pub(crate) use helpers::bytes;
pub use types::Lab;

#[cfg(test)]
mod tests;
