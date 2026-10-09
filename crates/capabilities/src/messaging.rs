//! The `messaging` capability, v1: the entities, the provider trait, the contract every provider passes, and a provider
//! that lives in memory. The spec is `docs/capabilities/messaging-v1.md`; the schema is
//! `docs/capabilities/messaging.schema.json`.

pub mod contract;
mod helpers;
mod memory;
mod structs;
mod traits;
mod types;

pub use helpers::{
    message_ref, origin_of, person_ref, require_message, split_message, workspace_ref,
};
pub use memory::MemoryMessaging;
pub use structs::{
    Attachment, Channel, ChannelQuery, Envelope, Event, Filter, Message, MessagingCapabilities,
    NewMessage, Page, Person, Reaction, SearchQuery, Workspace,
};
pub use traits::MessagingProvider;
pub use types::{
    AttachmentKind, ChannelKind, EntityKind, EventKind, Feature, Formatting, Operation,
};

#[cfg(test)]
mod tests;
