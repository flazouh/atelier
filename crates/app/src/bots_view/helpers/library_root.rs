use std::path::{Path, PathBuf};

use super::super::consts::FOLDER;

/// Where the bots live: the bots folder next to the settings file. No settings file, no folder.
pub fn library_root(settings: Option<&Path>) -> Option<PathBuf> {
    Some(settings?.parent()?.join(FOLDER))
}
