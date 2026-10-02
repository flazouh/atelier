/// How a session shows the tool calls of a turn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolDensity {
    /// A run of calls folds into one row of counts ("edited 1 · searched 2 · read 4"); open it to see the calls.
    #[default]
    Grouped,
    /// Each call has a row of its own, shut: its name and its target.
    Lines,
    /// Each call has a row of its own, open on its detail: an edit's diff as it is written, a command's output.
    Detailed,
}
