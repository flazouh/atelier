use atelier_capabilities::Actor;
use serde_json::Value;

use super::structs::{ToolDef, ToolResult};

/// The tools of one capability. The gateway asks each set in turn: the set whose `call` knows the name runs it.
///
/// A call blocks and may be slow, as the providers behind it are. The gateway runs each on its own thread.
pub trait ToolSet: Send + Sync {
    /// The tools to list, in the order to show them.
    fn tools(&self) -> Vec<ToolDef>;

    /// Runs `tool` for the session of `actor`, or gives `None` when the name is not this set's. Every change the
    /// call makes is attributed to `actor`.
    fn call(&self, tool: &str, arguments: &Value, actor: &Actor) -> Option<ToolResult>;
}
