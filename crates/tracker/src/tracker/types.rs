use serde::{Deserialize, Serialize};

use crate::TaskId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackerError {
    /// No task has this id.
    NotFound(TaskId),
    /// The request cannot be made sense of: an empty title, a task that is its own parent.
    Invalid(String),
    /// The store failed: the database is locked, the disk is full, a newer app wrote it.
    Storage(String),
    /// This tracker cannot do that. A backend says so for what its service has no place for.
    Unsupported(String),
}

impl std::fmt::Display for TrackerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "no task {id}"),
            Self::Invalid(why) => write!(f, "{why}"),
            Self::Storage(why) => write!(f, "the task store failed: {why}"),
            Self::Unsupported(what) => write!(f, "this tracker cannot {what}"),
        }
    }
}

impl std::error::Error for TrackerError {}

pub type TrackerResult<T> = Result<T, TrackerError>;
