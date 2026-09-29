//! A process a project starts: a language server, a git run, later the agent. Its pipes are plain
//! byte streams, so a process on this machine and one on an SSH host (M1b) look the same.

use std::{
    io::{self, Read, Write},
    path::PathBuf,
    process::Child,
};

use serde::{Deserialize, Serialize};

/// What to start.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    /// A path on the host, or a name its `PATH` finds.
    pub program: PathBuf,
    pub args: Vec<String>,
    /// The host folder it runs in; the project's root when `None`.
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
}

impl Command {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self { program: program.into(), ..Self::default() }
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }
}

/// Stops and waits for a process.
pub trait Control: Send {
    fn kill(&mut self) -> io::Result<()>;
    /// Blocks until it ends; its exit code, or `None` when a signal ended it.
    fn wait(&mut self) -> io::Result<Option<i32>>;
    /// Whether it still runs, without blocking.
    fn running(&mut self) -> bool;
}

/// A started process: write to its stdin, read its stdout. Its stderr is dropped.
pub struct Process {
    pub stdin: Box<dyn Write + Send>,
    pub stdout: Box<dyn Read + Send>,
    pub control: Box<dyn Control>,
}

impl Control for Child {
    fn kill(&mut self) -> io::Result<()> {
        Child::kill(self)
    }

    fn wait(&mut self) -> io::Result<Option<i32>> {
        Child::wait(self).map(|status| status.code())
    }

    fn running(&mut self) -> bool {
        matches!(self.try_wait(), Ok(None))
    }
}
