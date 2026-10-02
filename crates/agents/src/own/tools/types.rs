/// What a call does to the project. Permission modes decide by this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// Looks only.
    Read,
    /// Changes files.
    Edit,
    /// Runs a process, which may do anything.
    Execute,
}

/// The most text a tool returns to the model. A longer result keeps its head and says so.
pub const MAX_RESULT: usize = 30_000;
