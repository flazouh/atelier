use serde::{Deserialize, Serialize};

use super::structs::Change;

/// The file of a project's tasks, in its data folder on its host.
pub const TRACKER_FILE: &str = "tracker.sqlite";

/// A change to the project's files and folders that the file tree offers. Paths are relative to the root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FsOp {
    NewFolder { path: String },
    /// Moves a file or a folder. Refused when `to` is there already.
    Rename { from: String, to: String },
    /// Copies a file, or a folder with all it holds. Refused when `to` is there already.
    Copy { from: String, to: String },
    /// Removes a file, or a folder with all it holds.
    Delete { path: String },
}

/// What happened to a path, as a watch reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChangeKind {
    Created,
    Changed,
    Removed,
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
