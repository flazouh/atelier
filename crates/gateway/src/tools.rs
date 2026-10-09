//! What a capability gives the gateway: a [`ToolSet`], which lists its tools and runs a call.
mod structs;
mod traits;
mod types;

pub use structs::{ToolDef, ToolResult};
pub use traits::ToolSet;
pub use types::Permission;
