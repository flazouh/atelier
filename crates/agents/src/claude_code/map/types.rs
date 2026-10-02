use std::time::Instant;

use crate::session::{BlockId, ToolId};

pub(in super::super) const ALLOW: &str = "allow";

pub(in super::super) const ALLOW_ALWAYS: &str = "allow_always";

pub(in super::super) const DENY: &str = "deny";

pub(super) enum Open {
    Text(BlockId),
    Thinking(BlockId, Instant),
    /// A tool call announced at the start of its block. `json` is its input so far; `targeted` is
    /// whether its file has been told; `shown` is the last edit told, for a tool whose text streams.
    Tool { id: ToolId, name: String, json: String, targeted: bool, shown: Option<crate::session::FileEdit> },
}
