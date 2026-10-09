//! The Mail pane: the threads of a mailbox of any mail provider, and the thread the reader opened with its reply box. It reads and
//! writes through the `MailProvider` of the account the reader chose, off the UI thread, and keeps its own copy of what it
//! shows. What the provider cannot do has no control, and each way a call fails has its own state.
//!
//! Three rules hold everywhere in it. Mail is data: every word of a sender is drawn as text, never as markup, and an address in
//! a body opens only when the reader presses it and has read where it goes. Opening a thread does not mark it read: the reader
//! chooses that, or the provider says it already did. Nothing is sent but the draft the reader sees, at the version they see.
mod helpers;
mod impls;
mod structs;
mod types;

#[cfg(test)]
use helpers::react;
pub use structs::{Account, MailPane};
pub use types::{MailPaneEvent, Problem};
#[cfg(test)]
use types::{POLL, Reaction};

#[cfg(test)]
mod tests;
