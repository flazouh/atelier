//! The Project interface: everything atelier does to a project's files, processes and git goes through
//! [`Project`], so the same app works on a folder on this machine ([`LocalProject`]) and on one over
//! SSH (M1b), with no code above the trait that knows which.
//!
//! Every call blocks and may be slow (a large tree, a slow link), so none is ever made on the UI
//! thread. Paths in the interface are relative to the root, with `/` between parts; `spawn` takes the
//! host's own paths, since a process runs there.

mod data;
mod helpers;
mod local;
mod process;
mod structs;
mod tracker_slot;
mod traits;
mod types;
mod worktrees;

pub use data::{DataEntry, adopt_old_data};
pub use local::LocalProject;
pub use process::{Command, Control, Process, STDERR_KEEP, Tail};
pub use tracker_slot::TrackerSlot;

pub use helpers::{expand_home, host_path, outside_path, read_local_dir};
pub use structs::{Change, DirEntry, Entry, GitOutput, Match, Query, Watch};
pub use traits::Project;
pub use types::{ChangeKind, ChangeSink, FsOp, Link, LinkSink, TRACKER_FILE};
pub use worktrees::{TreeState, Upstream, Worktree, worktrees};

