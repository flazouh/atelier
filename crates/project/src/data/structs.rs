use std::{
    fs,
    io,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};

use crate::{host_path, local::write_whole};
use super::types::DATA_NAME;
use super::helpers::{fnv1a, relative};

/// One file in a data folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataEntry {
    /// Relative to the data folder, `/` between parts.
    pub path: String,
    /// When it was last written, in milliseconds since 1970.
    pub modified_ms: u64,
}

pub struct DataFolder {
    pub(super) dir: PathBuf,
}

impl DataFolder {
    /// The folder of the project at `root`, under `data_dir`, or the platform's data folder.
    pub fn for_root(root: &Path, data_dir: Option<&Path>) -> Option<Self> {
        let data = match data_dir {
            Some(dir) => dir.to_path_buf(),
            None => match std::env::var_os("ATELIER_DATA_DIR") {
                Some(dir) => PathBuf::from(dir),
                None => dirs::data_dir()?.join(DATA_NAME),
            },
        };
        let name = root.to_string_lossy();
        let folder = root.file_name().and_then(|n| n.to_str()).unwrap_or("project");
        let readable: String = folder.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
        Some(Self { dir: data.join("projects").join(format!("{readable}-{:016x}", fnv1a(name.as_bytes()))) })
    }

    /// The folder itself.
    pub fn path(&self) -> &Path {
        &self.dir
    }

    pub fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        fs::read(host_path(&self.dir, path)?)
    }

    /// Writes the file whole, making its folders.
    pub fn write(&self, path: &str, bytes: &[u8]) -> io::Result<()> {
        let target = host_path(&self.dir, path)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        write_whole(&target, bytes)
    }

    /// Every file whose path is under `prefix` (a folder, or `""` for all), newest first.
    pub fn list(&self, prefix: &str) -> io::Result<Vec<DataEntry>> {
        let start = host_path(&self.dir, prefix)?;
        let mut found = Vec::new();
        let mut folders = vec![start];
        while let Some(folder) = folders.pop() {
            let entries = match fs::read_dir(&folder) {
                Ok(entries) => entries,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            for entry in entries {
                let entry = entry?;
                let meta = entry.metadata()?;
                if meta.is_dir() {
                    folders.push(entry.path());
                    continue;
                }
                let Some(path) = relative(&self.dir, &entry.path()) else { continue };
                let modified_ms = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64);
                found.push(DataEntry { path, modified_ms });
            }
        }
        found.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms).then_with(|| a.path.cmp(&b.path)));
        Ok(found)
    }
}
