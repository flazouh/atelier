//! The host's side: `atelier-remote --stdio` reads requests from stdin and answers on stdout, with a
//! [`LocalProject`](atelier_project::LocalProject) doing the work. Each request runs on a thread of its own, so a slow search never
//! holds up a read; a process's stdin is fed by a thread of its own, so its order holds and a
//! process that stops reading never holds up the pipe.

mod helpers;
mod structs;
mod tracker;
mod types;

pub use helpers::{serve, serve_with_data};
