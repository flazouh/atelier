//! Why the app ends, in its log (stderr): a panic and where, a signal and its name, or an exit with
//! no signal, such as a lost X display. An end that says nothing leaves nothing to fix.

mod helpers;
#[cfg(all(test, unix))]
mod tests;
mod types;

pub use helpers::{closed, install, last_window_closed, quit};

#[cfg(test)]
use helpers::words;
