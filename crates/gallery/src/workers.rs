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
            Arc::new(Workers::new(disk, Store::from_env(), READY, ASK))
        })
        .clone()
}
