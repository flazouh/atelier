//! A project's data folder: lathe's own files about a project, such as its agents' sessions and its
//! reviews, on the project's host and outside the repository, so they never show in `git status`.
//! `<data>/lathe/projects/<folder>-<hash of the root>/`, where `<data>` is the platform's data folder
//! (`~/Library/Application Support` on macOS, `$XDG_DATA_HOME` or `~/.local/share` elsewhere), or
//! `LATHE_DATA_DIR`. The same root always gets the same folder.

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};

use crate::{host_path, local::write_whole};

/// One file in a data folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataEntry {
    /// Relative to the data folder, `/` between parts.
    pub path: String,
    /// When it was last written, in milliseconds since 1970.
    pub modified_ms: u64,
}

pub struct DataFolder {
    dir: PathBuf,
}

impl DataFolder {
    /// The folder of the project at `root`, under `data_dir`, or the platform's data folder.
    pub fn for_root(root: &Path, data_dir: Option<&Path>) -> Option<Self> {
        let data = match data_dir {
            Some(dir) => dir.to_path_buf(),
            None => match std::env::var_os("LATHE_DATA_DIR") {
                Some(dir) => PathBuf::from(dir),
                None => dirs::data_dir()?.join("lathe"),
            },
        };
        let name = root.to_string_lossy();
        let folder = root.file_name().and_then(|n| n.to_str()).unwrap_or("project");
        let readable: String = folder.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
        Some(Self { dir: data.join("projects").join(format!("{readable}-{:016x}", fnv1a(name.as_bytes()))) })
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

fn relative(dir: &Path, path: &Path) -> Option<String> {
    let parts: Vec<&str> = path.strip_prefix(dir).ok()?.iter().filter_map(|p| p.to_str()).collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// FNV-1a, 64 bits: the same on every machine and every run, unlike the standard library's hasher.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3))
}
