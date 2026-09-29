//! A process a project starts: a language server, a git run, the agent. Its pipes are plain byte
//! streams, so a process on this machine and one on an SSH host (M1b) look the same.
//!
//! Its stderr is kept, not streamed: the last [`STDERR_KEEP`] bytes, read on a thread of its own so a
//! chatty process never blocks on a full pipe. When a process ends early, [`Control::stderr`] says
//! why in its own words.

use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    path::PathBuf,
    process::{Child, ChildStderr},
    sync::{Arc, Mutex},
    thread,
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
    /// The last of what it wrote to stderr, up to [`STDERR_KEEP`] bytes, as text.
    fn stderr(&self) -> String;
}

/// How much of a process's stderr is kept: enough for a crash's message and its backtrace.
pub const STDERR_KEEP: usize = 64 * 1024;

/// The tail of a stream, filled by a thread of its own.
#[derive(Clone, Default)]
pub struct Tail(Arc<Mutex<VecDeque<u8>>>);

impl Tail {
    /// Reads `stream` to its end on a new thread, keeping its last [`STDERR_KEEP`] bytes.
    pub fn follow(stream: impl Read + Send + 'static) -> Self {
        let tail = Self::default();
        let keep = tail.clone();
        let _ = thread::Builder::new().name("lathe-stderr".into()).spawn(move || {
            let mut stream = stream;
            let mut chunk = [0u8; 4096];
            while let Ok(n) = stream.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                keep.push(&chunk[..n]);
            }
        });
        tail
    }

    pub fn push(&self, bytes: &[u8]) {
        let mut kept = self.0.lock().unwrap_or_else(|p| p.into_inner());
        kept.extend(bytes);
        let over = kept.len().saturating_sub(STDERR_KEEP);
        kept.drain(..over);
    }

    pub fn text(&self) -> String {
        let kept = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let (a, b) = kept.as_slices();
        String::from_utf8_lossy(&[a, b].concat()).into_owned()
    }
}

/// A child process on this machine, with its stderr's tail.
pub struct LocalChild {
    pub child: Child,
    pub stderr: Tail,
}

impl LocalChild {
    pub fn new(mut child: Child) -> Self {
        let stderr = child.stderr.take().map(|e: ChildStderr| Tail::follow(e)).unwrap_or_default();
        Self { child, stderr }
    }
}

/// A started process: write to its stdin, read its stdout; its stderr's tail is on its control.
pub struct Process {
    pub stdin: Box<dyn Write + Send>,
    pub stdout: Box<dyn Read + Send>,
    pub control: Box<dyn Control>,
}

impl Control for LocalChild {
    fn kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }

    fn wait(&mut self) -> io::Result<Option<i32>> {
        self.child.wait().map(|status| status.code())
    }

    fn running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    fn stderr(&self) -> String {
        self.stderr.text()
    }
}
