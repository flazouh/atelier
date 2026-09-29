//! What the gallery remembers across launches: the theme in force. A small JSON file in the app's
//! data folder (`<data dir>/lathe/settings.json`). It is read once before the window opens, and written
//! on a background thread, so the UI thread never waits on the disk.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// The theme's name, as the picker lists it.
    pub theme: Option<String>,
}

/// Where the settings live: `LATHE_SETTINGS` when set, for tests and scripts, else the data folder.
pub fn path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("LATHE_SETTINGS") {
        return Some(PathBuf::from(path));
    }
    Some(dirs::data_dir()?.join("lathe").join("settings.json"))
}

/// The settings at `path`; the defaults when the file is missing or unreadable.
pub fn load(path: &Path) -> Settings {
    std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

/// Writes `settings` to `path`, making its folder. Call it off the UI thread.
pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(std::io::Error::other)?;
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests;
