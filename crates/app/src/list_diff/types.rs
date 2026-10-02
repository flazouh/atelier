/// One row of a session's list: an item of the conversation, or the files a finished turn changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Item(usize),
    Changes { turn: usize },
    /// Items `from..to`: a run of thinking, tool calls and subagents with two or more rows to draw, as one
    /// group.
    Activity { from: usize, to: usize },
}

/// What a row is for its entrance: the item it starts at, or a turn's card. An item that joins others in a group, or
/// a group that grows, is the same row; it entered once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Arrival {
    Item(usize),
    Card(usize),
}

impl Arrival {
    pub fn of(row: Row) -> Self {
        match row {
            Row::Item(ix) | Row::Activity { from: ix, .. } => Self::Item(ix),
            Row::Changes { turn } => Self::Card(turn),
        }
    }
}
