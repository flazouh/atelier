//! The app's side: [`RemoteProject`] is a [`Project`] whose every call is a request to
//! `atelier-remote` on the host, answered within a timeout.
//!
//! A connection comes from a [`Dial`], so the same client runs over the user's `ssh` in the app and
//! over an in-process pipe in the tests. When the connection drops, every call waiting on it fails at
//! once, the link reports [`Link::Down`], and the client dials again, backing off, for as long as it
//! lives. Once it is back it says hello again, restores the watch, and reports [`Link::Up`]. What the
//! app has not saved lives in the app, so a drop loses nothing.

mod helpers;
mod structs;
mod tracker;
mod types;

pub use structs::{Connection, RemoteProject, Timeouts};
pub use types::Dial;

use helpers::lock;
use structs::Shared;

#[cfg(test)]
use std::{
    io::{self},
    sync::{atomic::Ordering},
    thread,
};
#[cfg(test)]
use crate::protocol::{Call, Failure, Frame, VERSION, read_frame, write_frame};

#[cfg(test)]
mod tests;
