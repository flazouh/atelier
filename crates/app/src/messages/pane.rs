//! The Messages pane: the history of a channel of any messaging provider, one thread in the same pane, and the composer. It
//! reads and writes through the `MessagingProvider` of the account the reader chose, off the UI thread, and keeps its own
//! copy of what it shows. What the provider cannot do has no control, and each way a call fails has its own state.
mod helpers;
mod structs;
mod types;

pub use structs::{Account, MessagesPane};
pub use types::{MessagesEvent, Problem};
#[cfg(test)]
use helpers::{apply, react};
#[cfg(test)]
use types::{Load, POLL, Reaction, View};

#[cfg(test)]
mod tests;
