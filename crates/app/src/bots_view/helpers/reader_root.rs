use std::path::PathBuf;

use super::library_root;

/// The reader's own bots folder. A test writes only where `ATELIER_SETTINGS` points, never in the reader's folder.
pub fn reader_root() -> Option<PathBuf> {
    if cfg!(test) && std::env::var_os("ATELIER_SETTINGS").is_none() {
        return None;
    }
    library_root(atelier_settings::path().as_deref())
}
