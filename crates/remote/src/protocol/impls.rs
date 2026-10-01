use std::io::self;

use super::structs::Failure;
use super::types::FailureKind;

impl From<Failure> for io::Error {
    fn from(failure: Failure) -> Self {
        let kind = match failure.kind {
            FailureKind::NotFound => io::ErrorKind::NotFound,
            FailureKind::PermissionDenied => io::ErrorKind::PermissionDenied,
            FailureKind::InvalidInput => io::ErrorKind::InvalidInput,
            FailureKind::Other => io::ErrorKind::Other,
        };
        io::Error::new(kind, failure.message)
    }
}
