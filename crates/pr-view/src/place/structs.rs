#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Placement {
    /// `(row, index into the threads)`, in row order; several threads may share a row.
    pub rows: Vec<(usize, usize)>,
    /// On the whole file.
    pub file_level: Vec<usize>,
    /// Their code changed since; no line to hang on.
    pub outdated: Vec<usize>,
}

impl Placement {
    pub fn total(&self) -> usize {
        self.rows.len() + self.file_level.len() + self.outdated.len()
    }
}
