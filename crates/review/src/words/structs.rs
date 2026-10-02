use std::ops::Range;

/// A pair of rows, one removed and one added, with the byte ranges that differ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowChange {
    /// Index into the removed rows of the hunk.
    pub removed_row: usize,
    /// Index into the added rows of the hunk.
    pub added_row: usize,
    pub removed: Vec<Range<usize>>,
    pub added: Vec<Range<usize>>,
}
