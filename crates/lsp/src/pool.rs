//! The running language servers: one per (server, project root), shared by every file in that
//! project, and started again if it stopped. This is how an editor asks for "the server for this
//! file" without knowing which language the file is in.

mod structs;
mod types;

pub use structs::Workers;
pub use types::NoServer;

#[cfg(test)]
use std::{path::Path, time::Duration};
#[cfg(test)]
use crate::Store;

#[cfg(test)]
mod tests;
