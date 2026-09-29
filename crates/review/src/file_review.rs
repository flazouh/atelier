//! One file's part in a turn: what it was, what it is, and the hunks between.
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::merged::Merged;

/// What happened to a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Modified,
    Added,
    Deleted,
    /// Moved with its text as it was; an edit as well shows as a delete and an add.
    Renamed { from: String },
}

/// What can be said about the file's text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Content {
    /// Text on both sides, with its hunks.
    Text(Merged),
    /// Not text: listed, with no hunks.
    Binary,
    /// The file changed but its text before the turn is not known: it was already changed when the turn
    /// started, and a command changed it further.
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileReview {
    /// Relative to the project, `/` between parts.
    pub path: String,
    pub change: Change,
    pub content: Content,
    /// The text before the turn; `None` when the file did not exist or is not text.
    pub before: Option<String>,
    /// The text now; `None` when the file is gone or is not text.
    pub after: Option<String>,
    /// The text before the turn is the file's own. Not exact when it stands in for text lost: the
    /// last commit's text of a file that already held changes when the turn started.
    pub exact: bool,
    version: u64,
}

impl FileReview {
    /// A file from its texts: `before` and `after` are `None` for a file that did not exist or is gone, and
    /// the change follows from which is missing. `exact` says `before` is the file's own text.
    pub fn from_texts(path: impl Into<String>, before: Option<String>, after: Option<String>, exact: bool) -> Self {
        let change = match (&before, &after) {
            (None, _) => Change::Added,
            (_, None) => Change::Deleted,
            _ => Change::Modified,
        };
        let merged = Merged::diff(before.as_deref().unwrap_or(""), after.as_deref().unwrap_or(""));
        let version = hash_of(after.as_deref());
        Self { path: path.into(), change, content: Content::Text(merged), before, after, exact, version }
    }

    pub(crate) fn binary(path: impl Into<String>, change: Change) -> Self {
        Self { path: path.into(), change, content: Content::Binary, before: None, after: None, exact: true, version: 0 }
    }

    pub(crate) fn unknown(path: impl Into<String>, after: Option<String>) -> Self {
        let version = hash_of(after.as_deref());
        Self { path: path.into(), change: Change::Modified, content: Content::Unknown, before: None, after, exact: false, version }
    }

    /// A pure move: the same text under a new name.
    pub(crate) fn renamed(mut self, from: String) -> Self {
        self.change = Change::Renamed { from };
        self
    }

    /// Rows added and removed; nothing for a file with no text to count.
    pub fn counts(&self) -> (usize, usize) {
        match &self.content {
            Content::Text(merged) => merged.counts(),
            Content::Binary | Content::Unknown => (0, 0),
        }
    }

    /// A number that changes when the file's text now changes. Reviewed state is kept against it.
    pub fn version(&self) -> u64 {
        self.version
    }
}

fn hash_of(text: Option<&str>) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests;
