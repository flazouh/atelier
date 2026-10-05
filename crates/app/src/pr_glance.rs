//! What a pull request chip's card reads once it opens. The card draws the chip's facts at once; the
//! project whose repository it names then reads the pull request, its checks and its files in parallel,
//! off the UI thread, and the first failing check's log for the line that says why. While the card
//! stays open and its Live part shows, it reads again every ten seconds; closing it stops the reads.
//!
//! The card's buttons come here too: Merge after its confirmation, Approve, Ask the agent to fix (the
//! session that made the pull request, else a new one), the session, the log, and the card's Settings.
//! The parts the reader hides are kept as `pr_card_off`.

mod helpers;
mod structs;
mod types;

pub use helpers::{install, register};
#[cfg(test)]
pub(crate) use helpers::opened;
pub use structs::Reading;

#[cfg(test)]
mod tests;
