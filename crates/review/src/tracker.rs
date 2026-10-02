//! The record of one turn: the text of each file the agent touches, taken before its edit lands, and at
//! the turn's end the files that changed and their hunks. Two things name a touched file: a tool call
//! that says which file it edits, and git, which finds what a shell command changed.

mod helpers;
mod structs;
mod types;

pub use structs::TurnTracker;

#[cfg(test)]
mod tests;
