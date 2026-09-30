//! What a file of the review keeps for a commit: its text before the turn with the hunks the reader
//! accepted and the edits they made in them (`Merged::baseline`). Undecided and rejected hunks stay out.
//! Pure.

use lathe_review::{FileReview, Merged};

/// One file's part of a commit: its new text, or `None` when the file goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    pub path: String,
    pub text: Option<String>,
}

/// What `file` keeps with its hunks as `merged` holds them now; `None` when it keeps nothing, or has no
/// text to review.
pub fn kept(file: &FileReview, merged: Option<&Merged>) -> Option<Kept> {
    let text = merged?.baseline();
    let path = file.path.clone();
    match &file.before {
        // A new file: kept when the reader accepted some of it.
        None => (!text.is_empty()).then_some(Kept { path, text: Some(text) }),
        // A deletion accepted whole: the file goes.
        Some(_) if file.after.is_none() && text.is_empty() => Some(Kept { path, text: None }),
        Some(before) => (text != *before).then_some(Kept { path, text: Some(text) }),
    }
}

#[cfg(test)]
mod tests;
