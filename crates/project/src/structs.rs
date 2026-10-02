use serde::{Deserialize, Serialize};

pub use super::process::Control;
use super::types::ChangeKind;

/// One file or folder in the tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Relative to the root, `/` between parts.
    pub path: String,
    pub dir: bool,
}

/// One thing in a folder that is listed by its path on the host, for a folder picker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub dir: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub kind: ChangeKind,
}

/// Watching lasts as long as this lives.
pub struct Watch(#[allow(dead_code)] pub(super) Box<dyn Send>);

impl Watch {
    pub fn new(keep: impl Send + 'static) -> Self {
        Self(Box::new(keep))
    }
}

/// What to look for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Query {
    pub pattern: String,
    /// A regular expression rather than literal text.
    pub regex: bool,
    pub case_sensitive: bool,
    /// Stop after this many matching lines.
    pub limit: usize,
}

/// One matching line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Match {
    pub path: String,
    /// From 0.
    pub line: usize,
    pub text: String,
}

/// What a `git` run printed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitOutput {
    /// `None` when it was killed.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl GitOutput {
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }
}
