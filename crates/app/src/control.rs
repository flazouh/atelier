//! A way to drive a running atelier without a pointer, for QA: the app listens on a Unix socket (readable by its
//! owner only), one JSON request per line and one JSON answer per line. A debug build listens by default, at
//! `atelier-<pid>.sock` in `$XDG_RUNTIME_DIR`; `ATELIER_CONTROL=<path>` picks the place (and turns it on in a
//! release build), and `ATELIER_CONTROL=off` turns it off.
//!
//! - `{"cmd":"state"}`: the open projects and sessions, each with its agent, its bot and the mood of the bot's face when it
//!   belongs to one, its status and the rows the list shows.
//! - `{"cmd":"new_session"}` or `{"cmd":"new_session","agent":"Cursor"}`: opens a session in the active project.
//!   `{"cmd":"new_session","bot":"dot"}` opens one that belongs to the bot kept under that id, on the bot's harness.
//! - `{"cmd":"send","text":"hello"}`: sends a message in the session in front, as the composer does.
//! - `{"cmd":"find","name":"limit-continue"}` and `{"cmd":"click","name":"limit-continue"}`: where an element marked with
//!   [`marked`] was last drawn, and a press on it. `{"cmd":"click","x":10,"y":20}` presses a point of the window; `"button":"right"` presses it with the other button.
//! - `{"cmd":"update","event":{"kind":"ready"}}`: tells the window one event of the platform updater, to see the update's
//!   chip and button (`state` then says `updates.state`).
//! - `{"cmd":"limit"}`: pretends the account of the session in front reached its weekly limit, to see the box.
//!
//! `tools/atelier-ctl.sh` is the client, and `tools/dev-qa.sh` starts an app to drive.

mod helpers;
mod marks;
mod types;

pub use helpers::{serve, socket_path};
pub use marks::{marked, marked_named};
#[cfg(test)]
pub(crate) use {helpers::press_in_steps, marks::find};
#[cfg(test)]
pub(crate) use helpers::state;

#[cfg(test)]
mod tests;
