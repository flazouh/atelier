//! The loop. One thread per session owns the conversation. A message starts a turn: call the model, show
//! its reply as it streams, run the tools it asks for (asking the reader first when the mode says so),
//! call the model again with the results, and stop when it stops asking. `Interrupt` sets a flag that the
//! model stream, the permission wait and the running tool all look at.

mod helpers;
mod structs;
mod types;

pub(super) use helpers::{open, root_name};
