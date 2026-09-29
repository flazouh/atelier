//! What a turn changed, and the turns of a session.
use std::collections::BTreeMap;

use crate::file_review::{Change, Content, FileReview};

/// The files one turn changed, by path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TurnReview {
    files: Vec<FileReview>,
}

impl TurnReview {
    pub(crate) fn new(mut files: Vec<FileReview>) -> Self {
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Self { files }
    }

    pub fn files(&self) -> &[FileReview] {
        &self.files
    }

    pub fn file(&self, path: &str) -> Option<&FileReview> {
        self.files.binary_search_by(|f| f.path.as_str().cmp(path)).ok().map(|i| &self.files[i])
    }
}

/// Every turn of a session, in order.
#[derive(Clone, Debug, Default)]
pub struct SessionReview {
    turns: Vec<TurnReview>,
}

impl SessionReview {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a finished turn and gives its index.
    pub fn push(&mut self, turn: TurnReview) -> usize {
        self.turns.push(turn);
        self.turns.len() - 1
    }

    pub fn turns(&self) -> &[TurnReview] {
        &self.turns
    }

    /// The session as one change: each file against its text before the first turn that touched it, and
    /// as it was left by the last. A file a later turn put back the way it was is not in it. Because a
    /// turn's baseline is the text the user kept, what the user rejected in an earlier turn is not in it
    /// either.
    pub fn whole(&self) -> Vec<FileReview> {
        struct Span {
            before: Option<String>,
            after: Option<String>,
            unknown: bool,
            binary: bool,
        }
        let mut spans: BTreeMap<&str, Span> = BTreeMap::new();
        for file in self.turns.iter().flat_map(|t| t.files()) {
            let span = spans.entry(&file.path).or_insert_with(|| Span {
                before: file.before.clone(),
                after: None,
                unknown: false,
                binary: false,
            });
            span.after = file.after.clone();
            span.unknown |= matches!(file.content, Content::Unknown);
            span.binary |= matches!(file.content, Content::Binary);
        }
        spans
            .into_iter()
            .filter_map(|(path, span)| {
                if span.binary {
                    let change = match (&span.before, &span.after) {
                        (None, _) => Change::Added,
                        (_, None) => Change::Deleted,
                        _ => Change::Modified,
                    };
                    return Some(FileReview::binary(path, change));
                }
                if span.unknown {
                    return Some(FileReview::unknown(path, span.after));
                }
                (span.before != span.after).then(|| FileReview::from_texts(path, span.before, span.after, true))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
