//! The conversation in the words every model API shares: messages made of blocks. A model client maps
//! these to its API's JSON and back; nothing above it knows a wire format. The blocks are also what the
//! session record keeps, so a saved session resumes exactly.

mod structs;
mod traits;
mod types;

pub use structs::{Cancel, Message, ModelRequest, Reply, Secret, TokenUsage, ToolDef};
pub use traits::Model;
pub use types::{Block, Delta, ModelError, Role, StopReason, Thinking};
