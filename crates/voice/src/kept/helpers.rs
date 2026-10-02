use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// `<data>/atelier/speech/waiting`.
pub fn dir() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("atelier").join("speech").join("waiting"))
}

/// Writes one recording into `dir` and gives back its file. The name starts with the time, so names sort oldest first.
pub fn save(dir: &Path, tag: &str, samples: &[f32]) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let mut n = 0;
    let path = loop {
        let path = dir.join(format!("{millis:016}-{n}.clip"));
        if !path.exists() {
            break path;
        }
        n += 1;
    };
    let mut bytes = Vec::with_capacity(4 + tag.len() + samples.len() * 4);
    bytes.extend_from_slice(&(tag.len() as u32).to_le_bytes());
    bytes.extend_from_slice(tag.as_bytes());
    for s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    // Written aside and then moved in, so a quit halfway leaves no torn recording.
    let part = path.with_extension("part");
    fs::write(&part, bytes)?;
    fs::rename(&part, &path)?;
    Ok(path)
}

/// One kept recording: its tag and its samples.
pub fn load(path: &Path) -> io::Result<(String, Vec<f32>)> {
    let bytes = fs::read(path)?;
    let bad = || io::Error::new(io::ErrorKind::InvalidData, "not a kept recording");
    let len = u32::from_le_bytes(bytes.get(..4).ok_or_else(bad)?.try_into().map_err(|_| bad())?) as usize;
    let tag = std::str::from_utf8(bytes.get(4..4 + len).ok_or_else(bad)?).map_err(|_| bad())?.to_string();
    let rest = &bytes[4 + len..];
    if rest.len() % 4 != 0 {
        return Err(bad());
    }
    let samples = rest.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
    Ok((tag, samples))
}

/// The recordings kept in `dir`, oldest first; none when there is no such folder.
pub fn list(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .map(|all| all.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "clip")).collect())
        .unwrap_or_default();
    found.sort();
    found
}
