use super::structs::{Cancel, ModelRequest, Reply};
use super::types::{Delta, ModelError};

/// The model behind the agent: one streaming call. It knows messages and tools, and no project.
pub trait Model: Send + Sync {
    /// Streams the reply into `sink` and returns it whole. Checks `cancel` while it waits and returns
    /// [`ModelError::Cancelled`] soon after it is set.
    fn stream(&self, request: &ModelRequest<'_>, sink: &mut dyn FnMut(Delta), cancel: &Cancel) -> Result<Reply, ModelError>;
}
