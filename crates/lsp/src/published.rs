//! The diagnostics a server published on its own, for a server that does not answer pulls.
//!
//! A published set is current for a document only if it describes the text the server has now. A
//! set that names a version is current from that version on. A set that names none cannot say which
//! text it is about, so it counts only if it arrived after the last change: every sync forgets the
//! document's set, and a set for the old text can never pass for one about the new text.

use std::{collections::HashMap, path::{Path, PathBuf}};

use lsp_types::Diagnostic;

struct Set {
    version: Option<i32>,
    diagnostics: Vec<Diagnostic>,
}

/// The newest published set for each document, by path.
#[derive(Default)]
pub struct Published {
    sets: HashMap<PathBuf, Set>,
}

impl Published {
    /// Forgets `path`'s set: the server has new text for it, and an old set is about the old text.
    pub fn synced(&mut self, path: &Path) {
        self.sets.remove(path);
    }

    /// Keeps a set the server published, unless a set with a later version is already kept.
    pub fn store(&mut self, path: PathBuf, version: Option<i32>, diagnostics: Vec<Diagnostic>) {
        let older = self.sets.get(&path).is_some_and(|kept| matches!((kept.version, version), (Some(k), Some(v)) if v < k));
        if !older {
            self.sets.insert(path, Set { version, diagnostics });
        }
    }

    /// The set for `path` if it describes the text the server has at `version`.
    pub fn current(&self, path: &Path, version: i32) -> Option<&[Diagnostic]> {
        let set = self.sets.get(path)?;
        set.version.is_none_or(|v| v >= version).then_some(set.diagnostics.as_slice())
    }
}

#[cfg(test)]
mod tests;
