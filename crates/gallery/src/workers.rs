//! The gallery's language servers: one pool for every story. Its files sit in fixture folders all over
//! the disk, so its project is the disk. A server the user has not installed is downloaded into
//! lathe's folder, unless `LATHE_OFFLINE` is set.

use std::sync::{Arc, OnceLock};

use lathe_editor::{ASK, READY};
use lathe_lsp::{Store, Workers};

pub fn workers() -> Arc<Workers> {
    static WORKERS: OnceLock<Arc<Workers>> = OnceLock::new();
    WORKERS
        .get_or_init(|| {
            let disk = Arc::new(lathe_project::LocalProject::open("/").expect("the disk opens"));
            Arc::new(Workers::new(disk, store(), READY, ASK))
        })
        .clone()
}

/// The servers the user has, or downloads them.
#[cfg(not(test))]
fn store() -> Store {
    Store::from_env()
}

/// In a test, no server: a real one answers from threads of its own, which GPUI's test scheduler
/// rejects. lathe-lsp's own tests prove the servers.
#[cfg(test)]
fn store() -> Store {
    Store::new(std::env::temp_dir().join("lathe-gallery-tests"), Vec::new(), true)
}
