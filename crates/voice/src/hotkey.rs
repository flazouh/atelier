//! The key that dictates: hold it to talk, tap it to keep talking hands-free, tap again to stop.
//!
//! [`Tracker`] turns what the key does into presses, and knows nothing of any platform, so every case is a plain test. On macOS
//! [`listen`] feeds it from the app's own events: GPUI does not tell the left Option key from the right one, and does not see
//! Fn at all, so the listener reads the key code and the device's own modifier bits. It watches only Atelier's windows, which
//! needs no permission.
//!
//! The feedback is instant, so a press is taken back ([`Action::Cancel`]) when the key turns out to be part of a shortcut:
//! another key or a click within [`TAKE_BACK`] of it.

mod helpers;
mod types;

pub use helpers::{Tracker, listen};
pub use types::{Action, Input, Key, TAKE_BACK};

#[cfg(test)]
mod tests;
