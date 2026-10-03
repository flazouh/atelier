//! A way to drive a running atelier without a pointer, for QA: the app listens on a Unix socket (readable by its
//! owner only), one JSON request per line and one JSON answer per line. A debug build listens by default, at
//! `atelier-<pid>.sock` in `$XDG_RUNTIME_DIR`; `ATELIER_CONTROL=<path>` picks the place (and turns it on in a
//! release build), and `ATELIER_CONTROL=off` turns it off.
//!
//! - `{"cmd":"state"}`: the open projects and sessions, each with its agent, status and the rows the list shows.
//! - `{"cmd":"new_session"}` or `{"cmd":"new_session","agent":"Cursor"}`: opens a session in the active project.
//! - `{"cmd":"send","text":"hello"}`: sends a message in the session in front, as the composer does.
//!
//! `tools/atelier-ctl.sh` is the client, and `tools/dev-qa.sh` starts an app to drive.

mod helpers;
mod types;

pub use helpers::{serve, socket_path};

#[cfg(test)]
mod tests;
