use std::{
    io::{self},
    };

/// Stops and waits for a process.
pub trait Control: Send {
    fn kill(&mut self) -> io::Result<()>;
    /// Blocks until it ends; its exit code, or `None` when a signal ended it.
    fn wait(&mut self) -> io::Result<Option<i32>>;
    /// Whether it still runs, without blocking.
    fn running(&mut self) -> bool;
    /// The last of what it wrote to stderr, up to [`STDERR_KEEP`] bytes, as text.
    fn stderr(&self) -> String;
}
