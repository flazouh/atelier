use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::types::{MailEventKind, MailFeature, MailOperation, MailboxKind, Role, SearchSyntax};
use crate::{Actor, AuthKind, Limits, Ref};

/// The mail address the credentials belong to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    #[serde(rename = "ref")]
    pub reference: Ref,
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A mailbox or a label.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mailbox {
    #[serde(rename = "ref")]
    pub reference: Ref,
    pub name: String,
    pub role: Role,
    pub kind: MailboxKind,
    pub unread: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

/// A person in `from`, `to`, `cc` or `bcc`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub address: String,
}

impl Contact {
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            name: None,
            address: address.into(),
        }
    }

    pub fn named(name: impl Into<String>, address: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            address: address.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flags {
    pub read: bool,
    pub starred: bool,
}

/// What an attachment is. The bytes come from `download_attachment`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    #[serde(rename = "ref")]
    pub reference: Ref,
    pub filename: String,
    pub mime: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default)]
    pub inline: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    #[serde(rename = "ref")]
    pub reference: Ref,
    pub thread: Ref,
    pub from: Contact,
    #[serde(default)]
    pub to: Vec<Contact>,
    #[serde(default)]
    pub cc: Vec<Contact>,
    #[serde(default)]
    pub bcc: Vec<Contact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<Contact>,
    pub subject: String,
    #[serde(default)]
    pub snippet: String,
    /// The body as plain text. It is what every screen and every agent sees.
    pub text: String,
    /// The sender's HTML, kept as it came. No screen renders it in v1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// Milliseconds since the epoch, UTC.
    pub date: i64,
    pub flags: Flags,
    #[serde(default)]
    pub mailboxes: Vec<Ref>,
    /// A small set: `message-id`, `in-reply-to`, `references`, `list-unsubscribe`, `date`.
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

/// What a list shows of a thread.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThreadSummary {
    #[serde(rename = "ref")]
    pub reference: Ref,
    pub subject: String,
    pub snippet: String,
    pub participants: Vec<Contact>,
    pub message_count: u32,
    /// How many messages are unread.
    pub unread: u32,
    pub starred: bool,
    pub has_attachments: bool,
    #[serde(default)]
    pub mailboxes: Vec<Ref>,
    /// The date of the newest message, in milliseconds.
    pub last_at: i64,
    pub version: String,
}

/// A thread with its messages, oldest first.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Thread {
    #[serde(flatten)]
    pub summary: ThreadSummary,
    pub messages: Vec<Message>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    #[serde(rename = "ref")]
    pub reference: Ref,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<Ref>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<Ref>,
    #[serde(default)]
    pub to: Vec<Contact>,
    #[serde(default)]
    pub cc: Vec<Contact>,
    #[serde(default)]
    pub bcc: Vec<Contact>,
    pub subject: String,
    pub text: String,
    pub created_by: Actor,
    pub created_at: i64,
    pub updated_at: i64,
    /// Opaque. Sent back on update and on send, so a changed text never goes under an old approval.
    pub version: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NewDraft {
    #[serde(default)]
    pub to: Vec<Contact>,
    #[serde(default)]
    pub cc: Vec<Contact>,
    #[serde(default)]
    pub bcc: Vec<Contact>,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<Ref>,
}

/// The fields to change. A field that is absent stays; an empty list clears the recipients.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DraftPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec<Contact>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cc: Option<Vec<Contact>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bcc: Option<Vec<Contact>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// A person's click on **Send**, for the exact draft they saw. An agent's `send` needs one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    pub person: Actor,
    /// The draft version on screen when the person clicked.
    pub version: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Read as the provider's `search_syntax` says.
    #[serde(default)]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mailbox: Option<Ref>,
    #[serde(default)]
    pub unread: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

impl SearchQuery {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MailEvent {
    pub kind: MailEventKind,
    pub thread: ThreadSummary,
}

/// A message that arrives. Real providers do not take it; a test, the memory provider and a fake service do.
#[derive(Clone, Debug, PartialEq)]
pub struct Incoming {
    pub from: Contact,
    pub to: Vec<Contact>,
    pub cc: Vec<Contact>,
    pub subject: String,
    pub text: String,
    pub html: Option<String>,
    pub date: i64,
    /// The thread it joins. Empty starts a new one.
    pub thread: Option<Ref>,
    /// Filename, mime type and bytes.
    pub attachments: Vec<(String, String, Vec<u8>)>,
}

impl Incoming {
    pub fn new(from: &str, to: &str, subject: &str, text: &str, date: i64) -> Self {
        Self {
            from: Contact::new(from),
            to: vec![Contact::new(to)],
            cc: vec![],
            subject: subject.into(),
            text: text.into(),
            html: None,
            date,
            thread: None,
            attachments: vec![],
        }
    }
}

/// What a mail provider can do. The screen and the agent tools offer only what is listed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailCapabilities {
    pub operations: Vec<MailOperation>,
    pub features: Vec<MailFeature>,
    pub search_syntax: SearchSyntax,
    pub limits: Limits,
    pub auth: Vec<AuthKind>,
}

impl MailCapabilities {
    pub fn can(&self, operation: MailOperation) -> bool {
        self.operations.contains(&operation)
    }

    pub fn has(&self, feature: MailFeature) -> bool {
        self.features.contains(&feature)
    }
}
