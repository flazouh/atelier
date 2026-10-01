//! What git says about the working tree, at the start of a turn and at its end. A shell command changes
//! files no tool call names; comparing the two states finds them.

mod helpers;
mod structs;
mod types;

pub(crate) use helpers::{head_files, snapshot};
#[cfg(test)]
pub(crate) use helpers::parse_status;
pub(crate) use structs::State;

#[cfg(test)]
mod tests;
