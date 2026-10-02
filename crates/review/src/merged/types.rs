/// Where a comment lands: on rows the agent wrote, or on rows it removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Rows of the file as it is now. Lines count from 1 in the current text.
    Current,
    /// Rows the agent removed. Lines count from 1 in the text before the turn.
    Removed,
}
