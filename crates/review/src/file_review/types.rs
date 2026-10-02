use crate::merged::Merged;

/// What happened to a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Modified,
    Added,
    Deleted,
    /// Moved with its text as it was; an edit as well shows as a delete and an add.
    Renamed { from: String },
}

/// What can be said about the file's text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Content {
    /// Text on both sides, with its hunks.
    Text(Merged),
    /// Not text: listed, with no hunks.
    Binary,
    /// The file changed but its text before the turn is not known: it was already changed when the turn
    /// started, and a command changed it further.
    Unknown,
}
