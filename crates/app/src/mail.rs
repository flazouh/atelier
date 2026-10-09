//! Mail in the app: one screen for the mailboxes, threads and messages of any mail provider. It is written once against the
//! `MailProvider` trait, as the Messages screen is against `MessagingProvider`, so Gmail, Outlook and IMAP need nothing of their
//! own here. The screen offers only what a provider lists, and never sends on its own: a person's click on Send sends the
//! draft shown. See `docs/capabilities/mail-v1.md`.
#[cfg(debug_assertions)]
pub mod demo;
pub mod map;
pub mod pane;
pub mod source;
