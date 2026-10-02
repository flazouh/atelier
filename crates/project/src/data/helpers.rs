use std::{fs, io, path::Path};

use super::types::{DATA_NAME, OLD_DATA_NAME};

/// Takes over the data of the version named lathe (`<data>/lathe`: settings, sessions, reviews, tasks) as
/// atelier's own (`<data>/atelier`), once: only when atelier has none yet. `<data>` is `data_dir`, or the
/// platform's data folder. Says whether it moved anything. Call it at start, before anything reads the data.
pub fn adopt_old_data(data_dir: Option<&Path>) -> io::Result<bool> {
    let Some(data) = data_dir.map(Path::to_path_buf).or_else(dirs::data_dir) else { return Ok(false) };
    let (old, new) = (data.join(OLD_DATA_NAME), data.join(DATA_NAME));
    if new.exists() || !old.is_dir() {
        return Ok(false);
    }
    fs::rename(old, new)?;
    Ok(true)
}

pub(super) fn relative(dir: &Path, path: &Path) -> Option<String> {
    let parts: Vec<&str> = path.strip_prefix(dir).ok()?.iter().filter_map(|p| p.to_str()).collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// FNV-1a, 64 bits: the same on every machine and every run, unlike the standard library's hasher.
pub(super) fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3))
}
