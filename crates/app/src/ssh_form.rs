//! "Open over SSH…": a host, typed or picked from the user's `~/.ssh/config` below the field, and Connect. The
//! folder is chosen next, in the folder picker over the host's own folders. Focus starts on the host; Enter
//! connects, and Escape closes the form. While it connects the form shows each step ("Reaching hp-agent…", "Putting atelier-remote
//! on hp-agent…"); a failure shows ssh's own words and leaves the form open to try again.

mod structs;
mod types;

pub use structs::SshForm;
pub use types::{Phase, SshFormEvent};

#[cfg(test)]
use types::HOST_CHIPS;

#[cfg(test)]
use gpui_kit::Entity;

#[cfg(test)]
mod tests;
