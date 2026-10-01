//! What goes over the pipe between the app and `atelier-remote`: frames of postcard bytes, each after
//! its length as four little-endian bytes. postcard is binary, so a file's bytes cross as they are,
//! and it is serde, so the frames reuse the Project interface's own types.
//!
//! The app sends [`Request`]s; the host answers each with a [`Response`] of the same id, and on its
//! own sends [`Event`]s: a watch's changes, a process's output, a process's end. The first request
//! is always [`Call::Hello`], which checks both ends speak this [`VERSION`].

mod helpers;
mod impls;
mod structs;
pub mod tracker;
mod types;

pub use helpers::{read_frame, write_frame};
pub use structs::Failure;
pub use types::{Call, Event, FailureKind, Frame, MAX_FRAME, Pid, Reply, STAMP, VERSION};

#[cfg(test)]
use std::io::self;

#[cfg(test)]
mod tests;
