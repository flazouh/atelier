//! What lathe remembers across launches: the theme in force and the recent projects. One small JSON
//! file in the app's data folder (`<data dir>/lathe/settings.json`), shared by the app and the gallery.
//! It is read once before the first window opens, and changed on a background thread, so the UI
//! thread never waits on the disk.
//!
//! A change loads the file, changes it and writes it back ([`update`]), and keys this version does not
//! know are kept, so the gallery setting the theme never drops the app's recent projects.

use std::{
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// How many recent projects the list keeps.
pub const RECENT_LIMIT: usize = 10;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The theme's name, as the picker lists it.
    pub theme: Option<String>,
    /// Newest first.
    pub recent: Vec<Location>,
    /// Keys a newer or older lathe wrote, kept as they are.
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

/// Where a project lives.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Location {
    Local { path: PathBuf },
    /// A folder on an SSH host, as `ssh` names the host (an alias from ~/.ssh/config works).
    Ssh { host: String, path: PathBuf },
}

impl Location {
    /// The folder's own name, for a list.
    pub fn name(&self) -> String {
        let path = match self {
            Location::Local { path } | Location::Ssh { path, .. } => path,
        };
        path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
    }

    /// Where it is, under the name: the folder's path, and the host for a remote one.
    pub fn place(&self) -> String {
        match self {
            Location::Local { path } => tilde(path),
            Location::Ssh { host, path } => format!("{host}:{}", path.display()),
        }
    }
}

/// `path` with the home folder as `~`.
fn tilde(path: &Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

impl Settings {
    /// Puts `location` first in the recent list, once, keeping at most [`RECENT_LIMIT`].
    pub fn opened(&mut self, location: Location) {
        self.recent.retain(|l| *l != location);
        self.recent.insert(0, location);
        self.recent.truncate(RECENT_LIMIT);
    }
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

/// Loads the settings at `path`, applies `change`, and writes them back whole. Call it off the UI
/// thread.
pub fn update(path: &Path, change: impl FnOnce(&mut Settings)) -> io::Result<Settings> {
    let mut settings = load(path);
    change(&mut settings);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&settings).map_err(io::Error::other)?;
    let temporary = path.with_extension("json.saving");
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, path)?;
    Ok(settings)
}

#[cfg(test)]
mod tests;
