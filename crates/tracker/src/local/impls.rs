use crate::TrackerError;

impl From<rusqlite::Error> for TrackerError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl From<serde_json::Error> for TrackerError {
    fn from(error: serde_json::Error) -> Self {
        Self::Storage(format!("a stored entry does not read: {error}"))
    }
}
