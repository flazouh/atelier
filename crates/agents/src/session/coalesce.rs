//! Sessions push events as fast as an agent streams them; the UI draws once a frame. The queue joins
//! the deltas of one block while events wait, and wakes the UI once per batch, so a stream of
//! thousands of tokens costs one repaint a frame.

mod helpers;
mod structs;

pub use structs::EventQueue;
