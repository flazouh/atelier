use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use atelier_tracker::{Tracker, TrackerError, TrackerResult};

pub use super::data::DataEntry;
pub use super::process::{Command, Process};
use super::structs::{DirEntry, Entry, GitOutput, Match, Query, Watch};
use super::types::{ChangeSink, FsOp, LinkSink};
use super::helpers::unsupported;

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
    /// The file at `path`, a path on the project's host that may lie outside its folder: absolute, with no `..`. Only
    /// the review uses it, for the files an agent changed beyond the project, and only for the ones it saw changed.
    fn read_outside(&self, path: &str) -> io::Result<Vec<u8>> {
        Err(unsupported("read_outside", path))
    }
    /// Writes the file at `path` whole, as [`Project::read_outside`] names it, and makes the folders above it.
    fn write_outside(&self, path: &str, _bytes: &[u8]) -> io::Result<()> {
        Err(unsupported("write_outside", path))
    }
    /// Removes the file at `path`, as [`Project::read_outside`] names it.
    fn remove_outside(&self, path: &str) -> io::Result<()> {
        Err(unsupported("remove_outside", path))
    }
    /// Makes a folder, moves, copies or removes, as the file tree asks.
    fn apply(&self, op: &FsOp) -> io::Result<()> {
        Err(unsupported("apply", &format!("{op:?}")))
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
    /// A file of the project's data folder: atelier's own data about this project, on the project's
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
    /// The entries of the folder `dir` on the project's host: an absolute path, or one that starts with `~/`. It
    /// need not be inside the project, so a picker can browse to a folder that is not a project yet.
    fn read_dir(&self, dir: &str) -> io::Result<Vec<DirEntry>> {
        Err(unsupported("read_dir", dir))
    }
    /// The data folder itself, as its host names it, for a tool that needs a real path there (git, tar): a
    /// bare repository is not a file to `data_write`. `None` when the project has no data folder, or its host
    /// cannot say where it is. The folder may not exist yet.
    fn data_path(&self) -> Option<PathBuf> {
        None
    }

    /// The project's tasks, kept on its host in its data folder ([`TRACKER_FILE`](crate::TRACKER_FILE)), so every machine that
    /// opens the project sees the same ones. The first call opens the store and later calls get the same
    /// one. It blocks, so it is never called on the UI thread.
    fn tracker(&self) -> TrackerResult<Arc<dyn Tracker>> {
        Err(TrackerError::Unsupported("keep tasks".into()))
    }

    /// The same project seen from `folder`, a worktree of its repository as the host names it (the main
    /// checkout is one too). Files, search, watch, git and spawn act there; the host, the link, the data
    /// folder and the tracker stay the project's. A folder that is not such a worktree is refused.
    fn at(&self, folder: &Path) -> io::Result<Arc<dyn Project>> {
        Err(unsupported("at", &folder.to_string_lossy()))
    }
}
