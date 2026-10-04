use std::time::Duration;

/// How long a watch gathers changes before it sends them, so a save that touches a file three times,
/// or a checkout that touches a thousand, arrives as one batch.
pub const WATCH_BATCH: Duration = Duration::from_millis(50);

/// A file longer than this is not searched: it is data, not source.
pub(super) const SEARCH_MAX_BYTES: u64 = 4 << 20;

/// Whether the platform's watch costs one handle per folder (inotify), so a watch registers the listed folders one by
/// one and leaves ignored ones out; elsewhere one recursive watch covers the tree.
pub(crate) const EACH_FOLDER: bool = cfg!(target_os = "linux");
