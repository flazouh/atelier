//! What goes over the pipe between the app and `atelier-remote`: frames of postcard bytes, each after
//! its length as four little-endian bytes. postcard is binary, so a file's bytes cross as they are,
//! and it is serde, so the frames reuse the Project interface's own types.
//!
//! The app sends [`Request`]s; the host answers each with a [`Response`] of the same id, and on its
//! own sends [`Event`]s: a watch's changes, a process's output, a process's end. The first request
//! is always [`Call::Hello`], which checks both ends speak this [`VERSION`].

use std::io::{self, Read, Write};

use atelier_project::{Change, Command, DataEntry, DirEntry, Entry, GitOutput, Match, Query};
use serde::{Deserialize, Serialize};

pub mod tracker;

use tracker::{TrackerCall, TrackerReply};

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

/// Why a call failed on the host: an `io::ErrorKind` by name, and the message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    pub kind: FailureKind,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureKind {
    NotFound,
    PermissionDenied,
    InvalidInput,
    Other,
}

impl From<&io::Error> for Failure {
    fn from(error: &io::Error) -> Self {
        let kind = match error.kind() {
            io::ErrorKind::NotFound => FailureKind::NotFound,
            io::ErrorKind::PermissionDenied => FailureKind::PermissionDenied,
            io::ErrorKind::InvalidInput => FailureKind::InvalidInput,
            _ => FailureKind::Other,
        };
        Self { kind, message: error.to_string() }
    }
}

impl From<Failure> for io::Error {
    fn from(failure: Failure) -> Self {
        let kind = match failure.kind {
            FailureKind::NotFound => io::ErrorKind::NotFound,
            FailureKind::PermissionDenied => io::ErrorKind::PermissionDenied,
            FailureKind::InvalidInput => io::ErrorKind::InvalidInput,
            FailureKind::Other => io::ErrorKind::Other,
        };
        io::Error::new(kind, failure.message)
    }
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

/// Writes `frame` whole and flushes it.
pub fn write_frame(out: &mut impl Write, frame: &Frame) -> io::Result<()> {
    let bytes = postcard::to_stdvec(frame).map_err(io::Error::other)?;
    let length = u32::try_from(bytes.len()).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a frame over 4GB"))?;
    out.write_all(&length.to_le_bytes())?;
    out.write_all(&bytes)?;
    out.flush()
}

/// The next frame, or `None` when the pipe closed between frames.
pub fn read_frame(input: &mut impl Read) -> io::Result<Option<Frame>> {
    let mut length = [0u8; 4];
    match input.read_exact(&mut length) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    let length = u32::from_le_bytes(length) as usize;
    if length > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("a frame of {length} bytes is over the limit")));
    }
    let mut bytes = vec![0u8; length];
    input.read_exact(&mut bytes)?;
    postcard::from_bytes(&bytes).map(Some).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests;
