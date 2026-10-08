//! The changelog: every release's notes, built into the app, newest first. The version in the title bar opens it, so it
//! needs no network and is there for the version that runs.
mod helpers;

pub use helpers::{notes_of, releases};

#[cfg(test)]
mod tests;
