use std::collections::HashMap;

/// A file as it was when the agent first touched it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Baseline {
    Text(String),
    /// The file did not exist.
    Absent,
    /// Not text, with a hash of its bytes (0 when they are not known).
    Binary(u64),
}

/// A file as it is now.
pub(super) enum Now {
    Text(String),
    Absent,
    Binary(u64),
}

/// The last commit's bytes of each file, by path.
pub(super) type Heads = HashMap<String, Option<Vec<u8>>>;
