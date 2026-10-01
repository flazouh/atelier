//! What a file of the review keeps for a commit: its text before the turn with the hunks the reader
//! accepted and the edits they made in them (`Merged::baseline`). Undecided and rejected hunks stay out.
//! Pure.

use atelier_review::{FileReview, Merged};

/// One file's part of a commit: its new text, or `None` when the file goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    pub path: String,
    pub text: Option<String>,
    /// The file's text before the turn (`None`: it was not there), which may hold the reader's own
    /// edits that no commit has.
    pub before: Option<String>,
}

/// What `file` keeps with its hunks as `merged` holds them now; `None` when it keeps nothing, or has no
/// text to review.
pub fn kept(file: &FileReview, merged: Option<&Merged>) -> Option<Kept> {
    let text = merged?.baseline();
    let path = file.path.clone();
    match &file.before {
        // A new file: kept when the reader accepted some of it.
        None => (!text.is_empty()).then_some(Kept { path, text: Some(text), before: None }),
        // A deletion accepted whole: the file goes.
        Some(before) if file.after.is_none() && text.is_empty() => Some(Kept { path, text: None, before: Some(before.clone()) }),
        Some(before) => (text != *before).then_some(Kept { path, text: Some(text), before: Some(before.clone()) }),
    }
}

/// Whether the file held the reader's own uncommitted edits before the turn, which the commit takes
/// too: its text before the turn differs from HEAD's (`None` on either side: not there).
pub fn own_edits(kept: &Kept, head: Option<&str>) -> bool {
    kept.before.as_deref() != head
}

/// What the commit changes in this file against its text in HEAD (`None`: not in HEAD): rows added and
/// removed, for the list the reader sees before committing.
pub fn against(head: Option<&str>, kept: &Kept) -> (usize, usize) {
    Merged::diff(head.unwrap_or(""), kept.text.as_deref().unwrap_or("")).counts()
}

#[cfg(test)]
mod tests;
