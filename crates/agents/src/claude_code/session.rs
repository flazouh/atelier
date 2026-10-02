//! A running `claude`: two threads around the process, so neither reading nor writing ever waits on the
//! other or on the caller. The reader turns lines into events; the writer sends what `send` queued.

mod helpers;
mod structs;

pub(super) use structs::ClaudeSession;
