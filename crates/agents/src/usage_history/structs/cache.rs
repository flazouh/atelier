use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use super::FileUsage;

/// What a parsed file is kept under, and what makes it stale.
pub(crate) struct CacheEntry {
    pub(crate) mtime: SystemTime,
    pub(crate) size: u64,
    pub(crate) usage: Arc<FileUsage>,
}

/// Parsed files, kept by the caller between reads so an unchanged file (same path, mtime and size) is not parsed
/// again. It does not depend on the day or the UTC offset of a read.
#[derive(Default)]
pub struct Cache {
    pub(crate) entries: HashMap<PathBuf, CacheEntry>,
    pub(crate) parsed_last_read: usize,
}
