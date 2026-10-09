//! The `mail` tools: `mail_mailboxes`, `mail_search`, `mail_thread`, `mail_get` and `mail_create_draft`, over the
//! providers in a [`Registry`](atelier_capabilities::Registry). The spec is section 8 of `docs/capabilities/mail-v1.md`.
//!
//! No tool sends mail. An agent cannot send alone: it makes a draft, and the person clicks **Send** on the exact draft
//! (`MailProvider::send` takes the person's `Approval`, which only the app can build). A tool is listed only while a
//! provider that can do it is connected.
mod helpers;
mod render;
mod schemas;
mod structs;

pub use structs::MailTools;
