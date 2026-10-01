use std::{
    fs,
    path::{Path, PathBuf},
};

use atelier_ui::{InlineHunk, RowMap};

use super::types::{CHANGES, UNCHANGED};
use super::helpers::shown;

/// One changed file: its text at the head, and each hunk as the head row its added rows start at,
/// the rows it removed there, and how many rows it added.
pub(super) struct Change {
    pub(super) path: &'static str,
    pub(super) head: &'static str,
    pub(super) hunks: &'static [(usize, &'static [&'static str], usize)],
}

/// A changed file as the review shows it.
pub struct Shown {
    pub path: &'static str,
    /// The head's rows with each hunk's removed rows above its added ones.
    pub text: String,
    pub hunks: Vec<InlineHunk>,
    pub rows: RowMap,
}

impl Shown {
    pub fn added(&self) -> usize {
        self.hunks.iter().map(|h| h.added.len()).sum()
    }

    pub fn removed(&self) -> usize {
        self.hunks.iter().map(|h| h.removed.len()).sum()
    }
}

/// The repository on disk, written fresh, and the pull request's changed files as shown.
pub struct Fixture {
    pub root: PathBuf,
    pub changed: Vec<Shown>,
}

impl Fixture {
    pub fn write() -> Self {
        let root = std::env::temp_dir().join("atelier-gallery-pr");
        let files = UNCHANGED.iter().copied().chain(CHANGES.iter().map(|c| (c.path, c.head)));
        for (path, text) in files {
            let path = root.join(path);
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&path, text);
        }
        let root = atelier_lsp::canonical(&root);
        Self { root, changed: CHANGES.iter().map(shown).collect() }
    }

    /// The changed file at `relative`, by its index.
    pub fn changed_at(&self, relative: &str) -> Option<usize> {
        self.changed.iter().position(|f| f.path == relative)
    }

    /// `path` relative to the repository, when it is in it.
    pub fn relative(&self, path: &Path) -> Option<String> {
        path.strip_prefix(&self.root).ok().map(|p| p.to_string_lossy().into_owned())
    }
}
