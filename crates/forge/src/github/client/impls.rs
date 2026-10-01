use crate::ForgeError;
use super::structs::GraphError;

impl From<GraphError> for ForgeError {
    fn from(error: GraphError) -> Self {
        match error.kind.as_deref() {
            Some("NOT_FOUND") => ForgeError::NotFound(error.message),
            Some("FORBIDDEN" | "INSUFFICIENT_SCOPES") => ForgeError::Denied(error.message),
            _ => ForgeError::Rejected(error.message),
        }
    }
}
