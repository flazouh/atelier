/// What a tool may do to the world. The gateway turns it into the hints a client shows before it asks the person
/// (`docs/capabilities/tasks-v1.md`, section 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    /// Reads only: list, get, search.
    Read,
    /// Adds or changes something that can be changed back: create, comment, update.
    Write,
    /// Removes something. The client always asks.
    Delete,
}

impl Permission {
    pub fn read_only(self) -> bool {
        self == Self::Read
    }

    pub fn destructive(self) -> bool {
        self == Self::Delete
    }
}
