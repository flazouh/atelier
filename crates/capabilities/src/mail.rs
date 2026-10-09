//! The `mail` capability, v1: the entities, the provider trait, the contract every provider passes, and a provider that
//! lives in memory. The spec is `docs/capabilities/mail-v1.md`; the schema is `docs/capabilities/mail.schema.json`.
pub mod contract;
mod helpers;
mod memory;
mod structs;
mod traits;
mod types;

pub use helpers::{
    attachment_ref, check_send_approval, draft_ref, fence, html_to_text, kind_of, local_id,
    mailbox_ref, message_ref, reply_recipients, reply_subject, thread_ref,
};
pub use memory::MemoryMail;
pub use structs::{
    Account, Approval, Attachment, Contact, Draft, DraftPatch, Flags, Incoming, MailCapabilities,
    MailEvent, Mailbox, Message, NewDraft, SearchQuery, Thread, ThreadSummary,
};
pub use traits::MailProvider;
pub use types::{
    MailEventKind, MailFeature, MailOperation, MailboxKind, RefKind, Role, SearchSyntax,
};

#[cfg(test)]
mod tests;
