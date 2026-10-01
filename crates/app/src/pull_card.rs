//! The session's pull request, one card above the composer. Its `watch`, shared with every card of the
//! same pull request, reads it off the UI thread at the pace `poll` sets, while a card of it is on
//! screen. The card's merge actions write to the forge only after the reader confirms a merge or a
//! branch delete, and the forge's refusals show in its own words.

mod helpers;
mod poll;
mod structs;
mod types;
mod watch;

pub use structs::PullCard;
pub use types::CardEvent;

#[cfg(test)]
mod tests;
