#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitError {
    /// The review kept nothing to commit.
    Nothing,
    /// A hook, or git itself, refused the commit: its words.
    Refused(String),
    /// The branch moved before the commit: nothing was committed.
    Moved,
    /// The branch moved during the commit: the commit (its sha) sits on top of another one it may undo.
    Raced(String),
    /// Git could not be run, or a step before the commit failed.
    Git(String),
}

impl std::fmt::Display for CommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nothing => f.write_str("The review kept nothing to commit"),
            Self::Refused(words) => write!(f, "The commit was refused: {words}"),
            Self::Moved => f.write_str("The branch moved while committing; try again"),
            Self::Raced(sha) => write!(
                f,
                "Another commit landed while committing: {sha} sits on top of it and may undo its changes. Check it with git show {sha} before you push"
            ),
            Self::Git(words) => write!(f, "Git failed: {words}"),
        }
    }
}
