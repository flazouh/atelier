//! Tool cards: a call of an agent to a tool of the gateway (`tasks_*`, `messaging_*`, `mail_*`) shows in the chat as a card
//! with the provider's tile and name, one line, and the result as rows, in place of the plain tool row. Each tool has a card
//! written once, as data in the card schema (`docs/capabilities/card-schema-v1.md`), that draws the neutral result of the
//! gateway for every provider. The result is the JSON text the agent got; a call with no card, or a result that is not the
//! shape its card draws, keeps the plain row. A card can only open what its result names: it never makes a call.
mod build;
mod cards;
mod map;
mod providers;
mod types;

pub(crate) use build::{card_for, now_ms};
pub(crate) use types::Press;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tests;
