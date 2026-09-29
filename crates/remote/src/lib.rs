//! The Project interface over a pipe. `lathe-remote --stdio` serves a host's folder ([`server`]);
//! the app speaks to it over the user's own `ssh` (M1b), through the frames in [`protocol`].

pub mod protocol;
pub mod server;
