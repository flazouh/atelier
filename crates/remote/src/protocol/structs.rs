use std::io::self;

use serde::{Deserialize, Serialize};

use super::types::FailureKind;

/// Why a call failed on the host: an `io::ErrorKind` by name, and the message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    pub kind: FailureKind,
    pub message: String,
}

impl From<&io::Error> for Failure {
    fn from(error: &io::Error) -> Self {
        let kind = match error.kind() {
            io::ErrorKind::NotFound => FailureKind::NotFound,
            io::ErrorKind::PermissionDenied => FailureKind::PermissionDenied,
            io::ErrorKind::InvalidInput => FailureKind::InvalidInput,
            _ => FailureKind::Other,
        };
        Self { kind, message: error.to_string() }
    }
}
