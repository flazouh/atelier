/// A slice of a long text, and how it sits in the whole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Cut {
    /// What fits.
    pub text: String,
    /// The length of the whole text, in characters.
    pub total: usize,
    /// The characters before `text`.
    pub from: usize,
    /// The characters up to the end of `text`.
    pub to: usize,
}

impl Cut {
    /// Whether the reader misses something: text before the slice or after it.
    pub(crate) fn is_cut(&self) -> bool {
        self.from > 0 || self.to < self.total
    }
}
