use atelier_project::{Change, Command, DataEntry, DirEntry, Entry, GitOutput, Match, Query};
use serde::{Deserialize, Serialize};

use super::tracker::{TrackerCall, TrackerReply};
use super::structs::Failure;

/// The protocol's version: both ends must agree, or the hello fails.
/// 2: `Remove` and the data folder's calls. 3: `DataPath`. 4: `ReadDir`. 5: `Tracker`.
pub const VERSION: u32 = 5;

/// The protocol, as bytes a helper binary carries, so the app reads a copy's protocol from the file
/// with no need to run it (it may be built for another machine). Keep it in step with [`VERSION`].
pub const STAMP: &[u8] = b"atelier-remote-protocol:5;";

/// A frame longer than this is refused, so a garbled length cannot ask for gigabytes.
pub const MAX_FRAME: usize = 256 << 20;

/// A process the host started, by the number it gave it.
pub type Pid = u64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Call {
    /// Opens the project at `root` on the host.
    Hello { version: u32, root: String },
    List,
    Read { path: String },
    Write { path: String, bytes: Vec<u8> },
    /// Starts sending [`Event::Changes`].
    Watch,
    Search { query: Query },
    Spawn { command: Command },
    /// Bytes for a process's stdin.
    Input { pid: Pid, bytes: Vec<u8> },
    /// Closes a process's stdin.
    CloseInput { pid: Pid },
    Kill { pid: Pid },
    /// The last of a process's stderr.
    Stderr { pid: Pid },
    Git { args: Vec<String> },
    Remove { path: String },
    /// The project's data folder, on the host.
    DataRead { path: String },
    DataWrite { path: String, bytes: Vec<u8> },
    DataList { prefix: String },
    /// Where the project's data folder is on the host: [`Reply::Text`], or `NotFound` when it has none.
    DataPath,
    /// The entries of a folder on the host, inside the project or not.
    ReadDir { dir: String },
    /// A call of the project's tracker, which the host opens in its data folder.
    Tracker(TrackerCall),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Reply {
    /// The host's name for the root, after it resolved it.
    Hello { root: String },
    Done,
    Entries(Vec<Entry>),
    Bytes(Vec<u8>),
    Matches(Vec<Match>),
    Spawned { pid: Pid },
    Text(String),
    Git(GitOutput),
    DataEntries(Vec<DataEntry>),
    DirEntries(Vec<DirEntry>),
    /// The tracker's answer, with its own error, so a missing task stays a missing task.
    Tracker(Box<Result<TrackerReply, atelier_tracker::TrackerError>>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureKind {
    NotFound,
    PermissionDenied,
    InvalidInput,
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    Changes(Vec<Change>),
    /// Bytes a process wrote to its stdout.
    Output { pid: Pid, bytes: Vec<u8> },
    /// A process's stdout closed and it ended, with its exit code.
    Exited { pid: Pid, code: Option<i32> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Frame {
    Request { id: u64, call: Call },
    Response { id: u64, result: Result<Reply, Failure> },
    Event(Event),
}
