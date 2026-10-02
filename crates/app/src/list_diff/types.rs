/// One row of a session's list: an item of the conversation, or the files a finished turn changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Item(usize),
    Changes { turn: usize },
    /// Items `from..to`: a run of thinking, tool calls and subagents with two or more rows to draw, as one
    /// group.
    Activity { from: usize, to: usize },
}
