//! `claude`'s lines to atelier's events. The mapper keeps what one session needs between lines: which
//! blocks stream, which tools run, the todo list, the questions waiting for an answer.

mod consts;
mod enums;
mod impls;
mod structs;
mod traits;

pub use structs::ClaudeLineMapper;
pub use traits::LineMapper;
