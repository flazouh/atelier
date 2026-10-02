use std::{
    io,
    path::{Path, PathBuf},
    sync::Mutex,
};

use super::structs::Settings;

/// `path` with the home folder as `~`.
pub(super) fn tilde(path: &Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// Where the settings live: `ATELIER_SETTINGS` when set, for tests and scripts, else the data folder.
pub fn path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ATELIER_SETTINGS") {
        return Some(PathBuf::from(path));
    }
    Some(dirs::data_dir()?.join("atelier").join("settings.json"))
}

/// The settings at `path`; the defaults when the file is missing or unreadable.
pub fn load(path: &Path) -> Settings {
    std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

/// Held from the load to the write of an [`update`], so saves made together each keep their change.
static SAVING: Mutex<()> = Mutex::new(());

/// Loads the settings at `path`, applies `change`, and writes them back whole. Saves in this process take
/// turns, and each writes a file of its own before it replaces the settings. Call it off the UI thread.
pub fn update(path: &Path, change: impl FnOnce(&mut Settings)) -> io::Result<Settings> {
    let _turn = SAVING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut settings = load(path);
    change(&mut settings);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&settings).map_err(io::Error::other)?;
    let temporary = path.with_extension(format!("json.saving.{}", std::process::id()));
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, path)?;
    Ok(settings)
}
