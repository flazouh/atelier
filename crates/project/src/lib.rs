//! The Project interface: everything lathe does to a project's files, processes and git goes through
//! [`Project`], so the same app works on a folder on this machine ([`LocalProject`]) and on one over
//! SSH (M1b), with no code above the trait that knows which.
//!
//! Every call blocks and may be slow (a large tree, a slow link), so none is ever made on the UI
//! thread. Paths in the interface are relative to the root, with `/` between parts; `spawn` takes the
//! host's own paths, since a process runs there.

use std::{
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

mod data;
mod local;
mod process;

pub use data::DataEntry;
pub use local::LocalProject;
pub use process::{Command, Control, Process, STDERR_KEEP, Tail};

/// One file or folder in the tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Relative to the root, `/` between parts.
    pub path: String,
    pub dir: bool,
}

/// What happened to a path, as a watch reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeKind {
    Created,
    Changed,
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub kind: ChangeKind,
}

/// Where a watch sends its batches. It runs on the watch's own thread.
pub type ChangeSink = Box<dyn Fn(Vec<Change>) + Send>;

/// Whether the project's host can be reached. A project on this machine is always up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    Up,
    /// The connection dropped, for this reason; the project is reconnecting.
    Down(String),
}

/// Hears the link go down and come back up, on a thread of the project's.
pub type LinkSink = Box<dyn Fn(Link) + Send>;

/// Watching lasts as long as this lives.
pub struct Watch(#[allow(dead_code)] Box<dyn Send>);

impl Watch {
    pub fn new(keep: impl Send + 'static) -> Self {
        Self(Box::new(keep))
    }
}

/// What to look for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Query {
    pub pattern: String,
    /// A regular expression rather than literal text.
    pub regex: bool,
    pub case_sensitive: bool,
    /// Stop after this many matching lines.
    pub limit: usize,
}

/// One matching line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Match {
    pub path: String,
    /// From 0.
    pub line: usize,
    pub text: String,
}

/// What a `git` run printed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitOutput {
    /// `None` when it was killed.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl GitOutput {
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }
}

pub trait Project: Send + Sync {
    /// The project's folder, as its host names it.
    fn root(&self) -> &Path;
    /// Every file and folder under the root, `.gitignore` respected, `.git` left out, sorted by path.
    fn list(&self) -> io::Result<Vec<Entry>>;
    fn read(&self, path: &str) -> io::Result<Vec<u8>>;
    /// Writes the file whole, so a reader never sees half of it.
    fn write(&self, path: &str, bytes: &[u8]) -> io::Result<()>;
    /// Removes the file at `path`; a folder is refused.
    fn remove(&self, path: &str) -> io::Result<()> {
        Err(unsupported("remove", path))
    }
    fn watch(&self, sink: ChangeSink) -> io::Result<Watch>;
    fn search(&self, query: &Query) -> io::Result<Vec<Match>>;
    fn spawn(&self, command: &Command) -> io::Result<Process>;
    fn git(&self, args: &[&str]) -> io::Result<GitOutput>;
    /// Tells `sink` each time the link to the project's host goes down or comes back. A project on
    /// this machine has no link, so it never calls it.
    fn on_link(&self, _sink: LinkSink) {}
    /// A remote project's host, as `ssh` names it; `None` for one on this machine.
    fn host(&self) -> Option<&str> {
        None
    }
    /// A file of the project's data folder: lathe's own data about this project, on the project's
    /// host and outside the repository (`data.rs`). Paths are relative to that folder; `..` and
    /// absolute paths are refused, as in `read` and `write`.
    fn data_read(&self, path: &str) -> io::Result<Vec<u8>> {
        Err(unsupported("data_read", path))
    }
    /// Writes a file of the data folder whole, making its folders.
    fn data_write(&self, path: &str, _bytes: &[u8]) -> io::Result<()> {
        Err(unsupported("data_write", path))
    }
    /// The data folder's files under `prefix` (a folder, or `""` for all), newest first.
    fn data_list(&self, prefix: &str) -> io::Result<Vec<DataEntry>> {
        Err(unsupported("data_list", prefix))
    }
    /// The data folder itself, as its host names it, for a tool that needs a real path there (git, tar): a
    /// bare repository is not a file to `data_write`. `None` when the project has no data folder, or its host
    /// cannot say where it is. The folder may not exist yet.
    fn data_path(&self) -> Option<PathBuf> {
        None
    }
}

/// What a project that keeps no data folder, or removes nothing, answers: a test's stand-in, say.
fn unsupported(call: &str, path: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, format!("this project has no {call} ({path})"))
}

/// The host path of a root-relative `path`. A path that climbs out of the root (`..`), or names a
/// host path itself, is refused: a project reads and writes inside its folder only.
pub fn host_path(root: &Path, path: &str) -> io::Result<PathBuf> {
    let mut at = root.to_path_buf();
    for part in path.split('/').filter(|p| !p.is_empty() && *p != ".") {
        if part == ".." || part.contains('\\') || Path::new(part).is_absolute() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("{path} is not inside the project")));
        }
        at.push(part);
    }
    Ok(at)
}
