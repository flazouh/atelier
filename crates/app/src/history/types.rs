use atelier_ui::DiffLine;
use gpui_kit::SharedString;

/// One commit of the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub sha: SharedString,
    pub short: SharedString,
    pub author: SharedString,
    /// When it was committed, in seconds since the Unix epoch.
    pub at: u64,
    pub subject: SharedString,
}

/// A file a commit changed, with its lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitFile {
    pub path: SharedString,
    pub lines: Vec<DiffLine>,
}

/// A commit in full: its whole message and what it changed, file by file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub sha: SharedString,
    pub message: SharedString,
    pub files: Vec<CommitFile>,
}

/// Something read from git off the UI thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Read<T> {
    Reading,
    Ready(T),
    /// What git said, for the reader.
    Failed(SharedString),
}
