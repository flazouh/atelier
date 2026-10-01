//! The review as the models atelier-ui draws.
use atelier_ui::{
    changed_files::{ChangedFile, FileChange},
    inline_review::InlineHunk,
};

use crate::file_review::{Change, Content, FileReview};

/// The list of changed files that `ChangedFiles` and `ChangedFileTree` take: `+a −r`, and whether the
/// file is new, gone or moved.
pub fn changed_files(files: &[FileReview]) -> Vec<ChangedFile> {
    files
        .iter()
        .map(|file| {
            let (added, removed) = file.counts();
            let change = match &file.change {
                Change::Modified => FileChange::Modified,
                Change::Added => FileChange::Added,
                Change::Deleted => FileChange::Deleted,
                Change::Renamed { from } => FileChange::Renamed { from: from.clone().into() },
            };
            ChangedFile::new(file.path.clone(), added, removed).change(change)
        })
        .collect()
}

/// The hunks of a file, in the form the inline review edits; none for a file with no text.
pub fn hunks(file: &FileReview) -> &[InlineHunk] {
    match &file.content {
        Content::Text(merged) => merged.hunks(),
        Content::Binary | Content::Unknown => &[],
    }
}

#[cfg(test)]
mod tests;
