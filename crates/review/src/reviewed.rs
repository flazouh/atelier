//! Which files of which turns the user has read. A mark holds for one version of a file: when the file
//! changes again the mark no longer counts, as GitQuiet's Reviewed State expires when its file changes.
use std::collections::HashMap;

use crate::file_review::FileReview;

#[derive(Clone, Debug, Default)]
pub struct Reviewed {
    /// The version of each file, by turn and path, when the user marked it.
    marks: HashMap<(usize, String), u64>,
}

impl Reviewed {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark(&mut self, turn: usize, file: &FileReview) {
        self.marks.insert((turn, file.path.clone()), file.version());
    }

    pub fn unmark(&mut self, turn: usize, path: &str) {
        self.marks.remove(&(turn, path.to_string()));
    }

    /// Whether the user marked this version of the file in this turn.
    pub fn is_reviewed(&self, turn: usize, file: &FileReview) -> bool {
        self.marks.get(&(turn, file.path.clone())) == Some(&file.version())
    }

    /// How many of a turn's files are marked, of how many.
    pub fn progress(&self, turn: usize, files: &[FileReview]) -> (usize, usize) {
        (files.iter().filter(|f| self.is_reviewed(turn, f)).count(), files.len())
    }
}

#[cfg(test)]
mod tests;
