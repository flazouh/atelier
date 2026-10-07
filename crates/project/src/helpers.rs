use std::{
    io,
    path::{Path, PathBuf},
};

use super::structs::DirEntry;

/// The entries of the host folder `dir` (an absolute path, or one that starts with `~/`), folders first and each
/// group by name without regard to case. A link to a folder counts as a folder. What cannot be read is left out.
pub fn read_local_dir(dir: &str) -> io::Result<Vec<DirEntry>> {
    let path = expand_home(dir).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("{dir} is not an absolute path")))?;
    let mut entries: Vec<DirEntry> = std::fs::read_dir(&path)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            let dir = std::fs::metadata(entry.path()).map(|m| m.is_dir()).unwrap_or(false);
            Some(DirEntry { name, dir })
        })
        .collect();
    entries.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())).then_with(|| a.name.cmp(&b.name)));
    Ok(entries)
}

/// `path` with a leading `~` made the home folder; `None` for a path that is not absolute after that.
pub fn expand_home(path: &str) -> Option<PathBuf> {
    let expanded = match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => {
            let home = std::env::var_os("HOME")?;
            PathBuf::from(home).join(rest.trim_start_matches('/'))
        }
        _ => PathBuf::from(path),
    };
    expanded.is_absolute().then_some(expanded)
}

/// What a project that keeps no data folder, or removes nothing, answers: a test's stand-in, say.
pub(super) fn unsupported(call: &str, path: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, format!("this project has no {call} ({path})"))
}

/// `path` as a host path, when it is absolute with no `..` or `.` part: the one shape the files beyond a project's folder
/// may be named in.
pub fn outside_path(path: &str) -> io::Result<PathBuf> {
    let at = Path::new(path);
    let plain = at.is_absolute() && !path.contains('\\') && path.split('/').all(|part| part != ".." && part != ".");
    if plain { Ok(at.to_path_buf()) } else { Err(io::Error::new(io::ErrorKind::InvalidInput, format!("{path} is not an absolute path"))) }
}

/// The host path of a root-relative `path`. A path that climbs out of the root (`..`), or names a
/// host path itself, is refused: a project reads and writes inside its folder only.
pub fn host_path(root: &Path, path: &str) -> io::Result<PathBuf> {
    let mut at = root.to_path_buf();
    for part in path.split('/').filter(|p| !p.is_empty() && *p != ".") {
        if part == ".." || part.contains('\\') || Path::new(part).is_absolute() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("{path} is not inside the project")));
        }
        at.push(part);
    }
    Ok(at)
}
