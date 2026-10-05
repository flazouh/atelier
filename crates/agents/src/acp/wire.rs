//! The payloads an ACP agent sends, as far as atelier reads them. A field atelier does not need is not here.
//! Kinds and statuses stay text and an update of a kind atelier does not know reads as `Other`, so a newer
//! agent never breaks an older atelier.

mod structs;
mod types;

pub(super) use structs::{
    ContextUsage, Initialized, Listed, Notification, Opened, PermissionAsked, PlanEntry, PromptUsage,
    Prompted, ToolCall,
};
pub(super) use types::{SessionUpdate, ToolContent};
