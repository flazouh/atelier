//! The Project interface over a pipe. `lathe-remote --stdio` serves a host's folder ([`server`]);
//! the app speaks to it over the user's own `ssh` (M1b), through the frames in [`protocol`].

pub mod client;
pub mod protocol;
pub mod server;
pub mod ssh;

pub use client::{Connection, Dial, RemoteProject, Timeouts};
