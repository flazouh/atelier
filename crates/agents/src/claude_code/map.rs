//! `claude`'s lines to atelier's events. The mapper keeps what one session needs between lines: which
//! blocks stream, which tools run, the todo list, the questions waiting for an answer. It reads no
//! clock and touches no process: the caller passes the time with each line.

mod helpers;
mod structs;
mod types;

pub use structs::Mapper;
