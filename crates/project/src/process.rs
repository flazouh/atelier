//! A process a project starts: a language server, a git run, the agent. Its pipes are plain byte
//! streams, so a process on this machine and one on an SSH host (M1b) look the same.
//!
//! Its stderr is kept, not streamed: the last [`STDERR_KEEP`] bytes, read on a thread of its own so a
//! chatty process never blocks on a full pipe. When a process ends early, [`Control::stderr`] says
//! why in its own words, complete the moment [`Control::wait`] returns.

mod helpers;
mod structs;
#[cfg(all(test, unix))]
mod tests;
mod traits;
mod types;

#[cfg(not(unix))]
pub use helpers::tether;
#[cfg(unix)]
pub use helpers::tether;
pub use structs::{Command, LocalChild, Process, Tail};
pub use traits::Control;
pub use types::STDERR_KEEP;

#[cfg(test)]
use helpers::tether_with;
#[cfg(test)]
use types::TETHER;
