//! One changed file, ready to draw: both sides in one text (each changed hunk as its old rows then its
//! new rows, the form the inline review edits), and the maps that put a thread or a language-server answer
//! on the right row. The diff itself is `lathe_review::Merged`; this only feeds it the two texts git gave.
use beui::{InlineHunk, RowMap};
use lathe_forge::Change;
use lathe_review::Merged;

use crate::git::{Blob, FileEntry};

/// What a file shows as.
#[derive(Clone, Debug)]
pub enum Content {
    Text(Box<Shown>),
    /// Not text: listed, no rows.
    Binary,
    TooLarge(u64),
}

#[derive(Clone, Debug)]
pub struct Shown {
    pub merged: Merged,
    pub lines: LineMap,
    /// Maps the head's rows to the shown ones, for the language server.
    pub rows: RowMap,
}

impl Shown {
    pub fn text(&self) -> &str {
        self.merged.text()
    }

    pub fn hunks(&self) -> &[InlineHunk] {
        self.merged.hunks()
    }

    pub fn counts(&self) -> (usize, usize) {
        self.merged.counts()
    }
}

#[derive(Clone, Debug)]
pub struct FileView {
    pub path: String,
    pub old_path: Option<String>,
    pub change: Change,
    /// Changes when the file's new text does. Reviewed State is kept against it.
    pub version: String,
    pub content: Content,
}

impl FileView {
    /// `old` and `new` are the two sides as git gave them.
    pub fn build(entry: &FileEntry, old: &Blob, new: &Blob) -> Self {
        let content = match (old, new) {
            (Blob::Binary, _) | (_, Blob::Binary) => Content::Binary,
            (Blob::TooLarge(size), _) | (_, Blob::TooLarge(size)) => Content::TooLarge(*size),
            _ if entry.binary => Content::Binary,
            _ => {
                let merged = Merged::diff(old.text_or_empty(), new.text_or_empty());
                let rows = RowMap::new(merged.hunks());
                let lines = LineMap::of(&merged);
                Content::Text(Box::new(Shown { merged, lines, rows }))
            }
        };
        Self { path: entry.path.clone(), old_path: entry.old_path.clone(), change: entry.change, version: entry.version().to_string(), content }
    }

    pub fn shown(&self) -> Option<&Shown> {
        match &self.content {
            Content::Text(shown) => Some(shown),
            _ => None,
        }
    }
}

/// The rows of a merged text, named in the lines of each side. A thread on the new side of line 12 sits
/// under the row that holds the head's line 12; one on the old side under the row that holds the old one.
#[derive(Clone, Debug, Default)]
pub struct LineMap {
    /// `head[i]` is the row of the head's line `i + 1`.
    head: Vec<usize>,
    /// `base[i]` is the row of the old file's line `i + 1`.
    base: Vec<usize>,
}

impl LineMap {
    pub fn of(merged: &Merged) -> Self {
        let rows = merged.text().matches('\n').count();
        let (mut head, mut base) = (Vec::with_capacity(rows), Vec::with_capacity(rows));
        // Hunks are in row order, so a cursor walks them once.
        let mut next = 0;
        for row in 0..rows {
            while next < merged.hunks().len() && merged.hunks()[next].added.end <= row {
                next += 1;
            }
            let (is_removed, is_added) = match merged.hunks().get(next) {
                Some(h) => (h.removed.contains(&row), h.added.contains(&row)),
                None => (false, false),
            };
            if !is_removed {
                head.push(row);
            }
            if !is_added {
                base.push(row);
            }
        }
        Self { head, base }
    }

    /// The row of the head's line `line` (from 1).
    pub fn head_row(&self, line: u32) -> Option<usize> {
        self.head.get((line as usize).checked_sub(1)?).copied()
    }

    /// The row of the old file's line `line` (from 1).
    pub fn base_row(&self, line: u32) -> Option<usize> {
        self.base.get((line as usize).checked_sub(1)?).copied()
    }

    /// The head's line (from 1) at `row`, or `None` for a row of removed text.
    pub fn head_line(&self, row: usize) -> Option<u32> {
        self.head.binary_search(&row).ok().map(|i| i as u32 + 1)
    }

    /// The old file's line (from 1) at `row`, or `None` for a row of added text.
    pub fn base_line(&self, row: usize) -> Option<u32> {
        self.base.binary_search(&row).ok().map(|i| i as u32 + 1)
    }
}
