//! Text arithmetic the review pane needs, pure: the one edit that turns the buffer's text into a new
//! one, where the caret lands after it, and the merged row a comment's line names.

use std::ops::Range;

use atelier_review::{Merged, Side};

/// The byte range of `old` to replace, and its replacement, that turn `old` into `new`: everything
/// between their common start and their common end. The agent writing a file again changes a few
/// rows, so the buffer takes one small edit instead of a whole new text, and keeps its caret and scroll.
pub fn splice<'a>(old: &str, new: &'a str) -> (Range<usize>, &'a str) {
    let mut start = old.bytes().zip(new.bytes()).take_while(|(a, b)| a == b).count();
    while !old.is_char_boundary(start) || !new.is_char_boundary(start) {
        start -= 1;
    }
    let most = old.len().min(new.len()) - start;
    let mut end = old.bytes().rev().zip(new.bytes().rev()).take(most).take_while(|(a, b)| a == b).count();
    while !old.is_char_boundary(old.len() - end) || !new.is_char_boundary(new.len() - end) {
        end -= 1;
    }
    (start..old.len() - end, &new[start..new.len() - end])
}

/// Where a caret at `caret` lands once `range` is replaced by `len` bytes: it keeps its place before the
/// edit, moves with the text after it, and goes to the end of the new text from inside the old.
pub fn moved_caret(caret: usize, range: &Range<usize>, len: usize) -> usize {
    if caret <= range.start {
        caret
    } else if caret >= range.end {
        caret - range.len() + len
    } else {
        range.start + len
    }
}

/// The row of the merged text that holds line `line` (from 1) of `side`'s version: the current text
/// skips the removed rows, and the text before the turn skips the added ones.
pub fn row_of(merged: &Merged, side: Side, line: u32) -> Option<usize> {
    let hunks = merged.hunks();
    let other = |row: usize| match side {
        Side::Current => hunks.iter().any(|h| h.removed.contains(&row)),
        Side::Removed => hunks.iter().any(|h| h.added.contains(&row)),
    };
    let rows = merged.text().split_inclusive('\n').count();
    (0..rows).filter(|row| !other(*row)).nth(usize::try_from(line).ok()?.checked_sub(1)?)
}

#[cfg(test)]
mod tests;
