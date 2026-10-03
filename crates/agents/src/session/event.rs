//! What a session tells the app. Every event is data the UI can draw without knowing the agent.

macro_rules! text_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

mod structs;
mod types;

pub use structs::{
    BlockId, Choice, ChoiceId, ContextFill, FileEdit, Limit, PermissionRequest, RequestId, SessionId, Started, Subagent, Todo,
    ToolCall, ToolId, ToolOutput, TurnEnd, Usage,
};
pub use types::{ChoiceKind, EndReason, Event, LimitState, LimitWindow, TodoStatus, ToolKind, ToolStatus, TurnOutcome};
