use std::{
    collections::{HashMap},
    rc::Rc,
    sync::Arc,
};

use gpui_kit::AnyElement;
use atelier_forge::{ForgeError, PullRef, PullState, Side, ThreadId};

use crate::{
    base::{Base, BaseChoice},
    checks::{JobLog},
    data::{Part, PullData},
    diff::FileView,
    git::{Blob, Commit, FileEntry, Prepared},
    sync::{Refreshed},
};

/// What the view tells its owner.
#[derive(Clone, Debug, PartialEq)]
pub enum PullEvent {
    /// The reader asked to open a file in the editor.
    OpenFile { pull: PullRef, path: String, line: Option<u32> },
    /// The pull request's state changed under the reader: merged, closed, opened again.
    StateChanged { pull: PullRef, state: PullState },
}

/// A file's diff, by where it came from: the base, the path, and the version at the head.
/// One block under a row: drawn again on each frame.
pub(super) type Block = Rc<dyn Fn() -> AnyElement>;

pub(crate) type FileKey = (String, String, String);

#[allow(clippy::large_enum_variant)]
pub(crate) enum Msg {
    Cached { data: Option<PullData>, marks: HashMap<String, String> },
    Part(Part),
    /// Every part of the first read has arrived.
    Loaded,
    Git { epoch: u64, prepared: Prepared, entries: Vec<FileEntry>, commits: Vec<Commit>, base: Base },
    GitFailed { epoch: u64, error: String },
    Checkout { epoch: u64, dir: Result<String, String> },
    File { key: FileKey, view: Arc<FileView> },
    FileFailed { key: FileKey, error: String },
    Brought { path: String, blob: Result<Blob, String> },
    Files { epoch: u64, files: Vec<String> },
    Job { id: u64, job: Option<JobLog> },
    Refreshed(Result<Refreshed, ForgeError>),
    Wrote(Result<String, ForgeError>),
}

#[derive(Clone)]
pub(super) enum DraftKind {
    Line { line: u32, side: Side },
    Reply(ThreadId),
}

pub(super) enum LookupKind {
    /// What each row means.
    Base(Vec<BaseChoice>),
    /// The paths of the rows, once the listing has arrived.
    Files(Vec<String>),
}

/// How many diffs stay in memory.
pub(super) const KEPT_FILES: usize = 24;
