//! The Gmail provider of the `mail` capability, v1. The spec is `docs/capabilities/mail-v1.md`.
//!
//! - [`GmailMail`]: the provider. It asks a [`Runner`] for JSON and maps it to the neutral mail types.
//! - [`Runner`]: how a call reaches Gmail. [`CliRunner`] runs `gmailcli ... -json`, on this machine or over SSH on the
//!   machine that holds the browser login. A test gives a fake that behaves like a small mailbox.
//!
//! `gmailcli` only reads, and it reads threads, not messages. So this provider lists `mailboxes`, `search`, `thread`, `get`,
//! `subscribe` (it polls) and, when the runner can fetch files, `download_attachment`. It lists nothing that writes: the
//! Gmail API provider is the way to drafts, send, labels and push. Read `GmailMail` for what each mapping loses.
mod helpers;
mod structs;
mod traits;
mod types;

pub use structs::{CliRunner, GmailMail};
pub use traits::Runner;
pub use types::RunFailure;

#[cfg(test)]
mod tests;
