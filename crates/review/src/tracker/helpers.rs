use std::{
    collections::{BTreeSet, HashMap},
    io::ErrorKind,
};

use atelier_project::Project;

use crate::{
    file_review::{Change, FileReview},
    git_state::{State},
};
use super::types::Now;

/// Whether the working tree holds something different at `path` than when the turn started.
pub(super) fn changed_since(start: &State, end: &State, path: &str) -> bool {
    match start.entries.get(path) {
        None => true,
        Some(was) => {
            let is = &end.entries[path];
            was.deleted != is.deleted || start.blobs.get(path) != end.blobs.get(path)
        }
    }
}

pub(super) fn read(project: &dyn Project, path: &str) -> Option<Now> {
    match project.read(path) {
        Ok(bytes) if bytes.contains(&0) => Some(Now::Binary(hash_bytes(&bytes))),
        Ok(bytes) => Some(match String::from_utf8(bytes) {
            Ok(text) => Now::Text(text),
            Err(error) => Now::Binary(hash_bytes(error.as_bytes())),
        }),
        Err(error) if error.kind() == ErrorKind::NotFound => Some(Now::Absent),
        Err(_) => None,
    }
}

pub(super) fn hash_bytes(bytes: &[u8]) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish().max(1)
}

/// A file gone and a file new with the same text is one file moved.
pub(super) fn pair_renames(files: Vec<FileReview>) -> Vec<FileReview> {
    let gone: HashMap<&str, &str> = files
        .iter()
        .filter(|f| f.change == Change::Deleted)
        .filter_map(|f| Some((f.before.as_deref()?, f.path.as_str())))
        .collect();
    let mut moved: HashMap<String, String> = HashMap::new();
    for file in files.iter().filter(|f| f.change == Change::Added) {
        if let Some(from) = file.after.as_deref().and_then(|text| gone.get(text)) {
            moved.insert(file.path.clone(), (*from).to_string());
        }
    }
    let sources: BTreeSet<String> = moved.values().cloned().collect();
    files
        .into_iter()
        .filter(|f| !sources.contains(&f.path))
        .map(|f| match moved.get(&f.path) {
            Some(from) => {
                let text = f.after.clone();
                FileReview::from_texts(f.path.clone(), text.clone(), text, true).renamed(from.clone())
            }
            None => f,
        })
        .collect()
}
