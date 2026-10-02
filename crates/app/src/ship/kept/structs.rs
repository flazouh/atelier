/// One file's part of a commit: its new text, or `None` when the file goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    pub path: String,
    pub text: Option<String>,
    /// The file's text before the turn (`None`: it was not there), which may hold the reader's own
    /// edits that no commit has.
    pub before: Option<String>,
}
