//! A project in a folder on this machine.

mod helpers;
mod structs;
mod types;

pub(crate) use helpers::write_whole;
pub use structs::LocalProject;

#[cfg(test)]
use std::io;
#[cfg(test)]
use crate::{Change, ChangeKind, Command, Match, Query};

#[cfg(test)]
mod tests;
