use std::{collections::HashMap, ops::Range};

use atelier_ui::inline_review::{Decision, InlineHunk, track_edit};
use imara_diff::{Algorithm, Diff, InternedInput};

use crate::lines::{RowTokens, Rows, join};
use super::types::Side;
use super::helpers::hunk_id;

/// The text before the agent's edit, the text after it, and the hunks between, as one text.
///
/// `text` holds every row once: a row both versions share appears as it is, and a changed hunk appears as
/// its old rows followed by its new rows, the rows [`InlineHunk`] names. Every row ends with a line
/// end here, so a hunk at the end of a file that lacked one does not run into the next row; the two
/// flags remember how each version really ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merged {
    pub(super) text: String,
    pub(super) hunks: Vec<InlineHunk>,
    baseline_final_newline: bool,
    current_final_newline: bool,
}

/// Rows of a merged text, named in the lines of the version they belong to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub side: Side,
    pub first_line: u32,
    pub last_line: u32,
    /// The rows themselves, joined by line ends.
    pub quote: String,
}

impl Merged {
    /// The hunks between `baseline` and `current`, line by line.
    pub fn diff(baseline: &str, current: &str) -> Self {
        let before = Rows::of(baseline);
        let after = Rows::of(current);
        let input = InternedInput::new(RowTokens(&before.rows), RowTokens(&after.rows));
        let mut diff = Diff::compute(Algorithm::Histogram, &input);
        diff.postprocess_lines(&input);

        let mut rows: Vec<&str> = Vec::with_capacity(before.rows.len().max(after.rows.len()));
        let mut hunks = Vec::new();
        let mut same: HashMap<u64, usize> = HashMap::new();
        let mut at = 0usize;
        for hunk in diff.hunks() {
            let (old, new) = (hunk.before.start as usize..hunk.before.end as usize, hunk.after.start as usize..hunk.after.end as usize);
            rows.extend(&before.rows[at..old.start]);
            let removed = rows.len()..rows.len() + old.len();
            rows.extend(&before.rows[old.clone()]);
            let added = rows.len()..rows.len() + new.len();
            rows.extend(&after.rows[new.clone()]);
            hunks.push(InlineHunk::new(hunk_id(&before.rows[old.clone()], &after.rows[new], &mut same), removed, added));
            at = old.end;
        }
        rows.extend(&before.rows[at..]);
        Self {
            text: join(rows, true),
            hunks,
            baseline_final_newline: before.final_newline,
            current_final_newline: after.final_newline,
        }
    }

    /// The merged text, for the editor buffer.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The hunks, in row order.
    pub fn hunks(&self) -> &[InlineHunk] {
        &self.hunks
    }

    /// Nothing changed.
    pub fn is_unchanged(&self) -> bool {
        self.hunks.is_empty() && self.baseline_final_newline == self.current_final_newline
    }

    /// Rows added and rows removed, all hunks together.
    pub fn counts(&self) -> (usize, usize) {
        self.hunks.iter().fold((0, 0), |(added, removed), h| (added + h.added.len(), removed + h.removed.len()))
    }

    /// The file as it was before the turn, or before the hunks still open.
    pub fn baseline(&self) -> String {
        self.version(|hunk| hunk.added.clone(), self.baseline_final_newline)
    }

    /// The file as it is now.
    pub fn current(&self) -> String {
        self.version(|hunk| hunk.removed.clone(), self.current_final_newline)
    }

    /// The rows outside `skipped(hunk)` for every hunk, joined.
    fn version(&self, skipped: impl Fn(&InlineHunk) -> Range<usize>, final_newline: bool) -> String {
        let skip: Vec<Range<usize>> = self.hunks.iter().map(skipped).collect();
        let mut next = 0;
        let kept = self.rows().enumerate().filter_map(|(row, text)| {
            while next < skip.len() && skip[next].end <= row {
                next += 1;
            }
            let inside = next < skip.len() && skip[next].contains(&row);
            (!inside).then_some(text)
        });
        join(kept, final_newline)
    }

    pub(super) fn rows(&self) -> impl Iterator<Item = &str> {
        let body = self.text.strip_suffix('\n').unwrap_or(&self.text);
        let empty = self.text.is_empty();
        (!empty).then(|| body.split('\n')).into_iter().flatten()
    }

    /// The user decided a hunk: its closing rows go, its surviving rows stay as plain code, and the other
    /// hunks move up. `None` when no hunk has that id.
    pub fn decide(&self, id: &str, decision: Decision) -> Option<Self> {
        let hunk = self.hunks.iter().find(|h| h.id == id)?;
        let closing = hunk.closing(decision);
        let last_row = self.rows().count();
        let touches_end = hunk.added.end == last_row;
        let mut kept = self.rows().enumerate().filter(|(row, _)| !closing.contains(row)).map(|(_, text)| text).peekable();
        let text = if kept.peek().is_none() { String::new() } else { join(kept, true) };
        let shift = |range: &Range<usize>| range.start - closing.len()..range.end - closing.len();
        let hunks = self
            .hunks
            .iter()
            .filter(|h| h.id != id)
            .map(|h| {
                if h.removed.start >= closing.end {
                    InlineHunk::new(h.id.clone(), shift(&h.removed), shift(&h.added))
                } else {
                    h.clone()
                }
            })
            .collect();
        // The rows that stay at the end carry the ending of the version they came from.
        let (mut baseline_final_newline, mut current_final_newline) = (self.baseline_final_newline, self.current_final_newline);
        if touches_end {
            match decision {
                Decision::Accept => baseline_final_newline = current_final_newline,
                Decision::Reject => current_final_newline = baseline_final_newline,
            }
        }
        Some(Self { text, hunks, baseline_final_newline, current_final_newline })
    }

    /// The user edited the merged text: the hunks stay on the rows they described.
    pub fn edited(&self, after: &str) -> Self {
        Self {
            text: after.to_string(),
            hunks: track_edit(&self.hunks, &self.text, after),
            baseline_final_newline: self.baseline_final_newline,
            current_final_newline: self.current_final_newline,
        }
    }

    /// The agent changed the file again while it is under review. What the user accepted is part of the
    /// baseline and what the user rejected is gone from the file, so the hunks of the new text are the
    /// ones still to decide, and a hunk that did not change keeps its id.
    pub fn rebased_on(&self, current: &str) -> Self {
        Self::diff(&self.baseline(), current)
    }

    /// The rows `rows` of the merged text, named in the lines of the version they belong to. The side is
    /// the side of the first row; the range ends where that side does. `None` when the range is empty or
    /// past the text.
    pub fn anchor(&self, rows: Range<usize>) -> Option<Anchor> {
        let all: Vec<&str> = self.rows().collect();
        if rows.is_empty() || rows.start >= all.len() {
            return None;
        }
        let removed = |row: usize| self.hunks.iter().any(|h| h.removed.contains(&row));
        let added = |row: usize| self.hunks.iter().any(|h| h.added.contains(&row));
        let side = if removed(rows.start) { Side::Removed } else { Side::Current };
        let belongs = |row: usize| match side {
            Side::Removed => removed(row),
            Side::Current => !removed(row),
        };
        let end = (rows.start..rows.end.min(all.len())).take_while(|row| belongs(*row)).last()? + 1;
        // Lines count in the version the rows belong to: rows of the other side do not count.
        let line_of = |row: usize| -> u32 {
            let before = (0..row).filter(|r| match side {
                Side::Removed => !added(*r),
                Side::Current => !removed(*r),
            });
            before.count() as u32 + 1
        };
        Some(Anchor { side, first_line: line_of(rows.start), last_line: line_of(end - 1), quote: all[rows.start..end].join("\n") })
    }
}
