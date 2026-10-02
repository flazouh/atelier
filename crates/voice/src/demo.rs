//! SPIKE: a made-up speech model and a made-up voice, so the dictation look can be seen before the real model is wired in.
//! The gallery and the app (behind `ATELIER_DICTATION_SPIKE`) both use it.

mod helpers;
mod types;

pub use helpers::{download_at, level};
pub use types::{DOWNLOAD, PREPARE, READY, TRANSCRIPT};

#[cfg(test)]
mod tests;
