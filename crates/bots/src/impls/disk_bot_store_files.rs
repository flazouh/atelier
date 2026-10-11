use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::enums::{BotsError, MemoryScope};
use crate::structs::BotId;

pub(super) fn io(path: &Path, e: std::io::Error) -> BotsError {
    BotsError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    }
}

/// Reads a JSON file. `None` when it does not exist.
pub(super) fn read<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, BotsError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(path, e)),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| BotsError::Parse {
            path: path.display().to_string(),
            reason: e.to_string(),
        })
}

/// Writes a JSON file whole: first to a file of its own, then in place, so a stop in the middle leaves the old file.
pub(super) fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), BotsError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| BotsError::Parse {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, text).map_err(|e| io(&temp, e))?;
    fs::rename(&temp, path).map_err(|e| io(path, e))
}

/// Removes the file kept under an id. A file that does not exist is not found.
pub(super) fn remove(path: &Path, id: &BotId) -> Result<(), BotsError> {
    fs::remove_file(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            BotsError::NotFound(id.to_string())
        } else {
            io(path, e)
        }
    })
}

/// The files in a folder that end in `.json`, by name. An absent folder holds none.
pub(super) fn json_files(dir: &Path) -> Result<Vec<PathBuf>, BotsError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(dir, e)),
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    Ok(files)
}

/// A name that is safe as a file name: letters and digits stay, every other byte becomes `_` and two hex digits.
pub(super) fn safe_name(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' {
            out.push(b as char);
        } else {
            out.push_str(&format!("_{b:02x}"));
        }
    }
    out
}

/// The folder and the file name that hold the notes of a layer.
pub(super) fn memory_file(root: &Path, scope: &MemoryScope) -> PathBuf {
    let (layer, key) = match scope {
        MemoryScope::Bot(id) => ("bot", safe_name(id.as_str())),
        MemoryScope::Workspace(id) => ("workspace", safe_name(id)),
        MemoryScope::Project(path) => ("project", safe_name(path)),
    };
    root.join("memory").join(layer).join(format!("{key}.json"))
}
