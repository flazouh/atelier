//! Recordings that wait for the model, kept on disk so quitting does not lose them. Each is one file: who it is for (the tag
//! the app gave the press), then the samples. A file goes when its words are out, or when the press is cancelled; the next
//! launch picks up whatever is left, oldest first.

mod helpers;

pub use helpers::{dir, list, load, save};

#[cfg(test)]
mod tests;
