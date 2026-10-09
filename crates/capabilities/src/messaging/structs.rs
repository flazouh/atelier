use serde::{Deserialize, Serialize};

use super::types::{
    AttachmentKind, ChannelKind, EntityKind, EventKind, Feature, Formatting, Operation,
};
use crate::{Actor, AuthKind, Limits, Ref};

/// What a messaging provider can do. The screen and the agent tools offer only what is listed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessagingCapabilities {
    pub operations: Vec<Operation>,
    pub features: Vec<Feature>,
    pub formatting: Formatting,
    pub limits: Limits,
    pub auth: Vec<AuthKind>,
}

impl MessagingCapabilities {
    /// The operations every messaging provider has.
    pub const CORE: [Operation; 5] = [
        Operation::Channels,
        Operation::History,
        Operation::Thread,
        Operation::Send,
        Operation::Subscribe,
    ];
    /// Every operation, for a check that walks them all.
    pub const ALL: [Operation; 13] = [
        Operation::Channels,
        Operation::History,
        Operation::Thread,
        Operation::Send,
        Operation::Subscribe,
        Operation::Search,
        Operation::Edit,
        Operation::Delete,
        Operation::React,
        Operation::MarkRead,
        Operation::Person,
        Operation::Export,
        Operation::Import,
    ];

    pub fn can(&self, operation: Operation) -> bool {
        self.operations.contains(&operation)
    }

    pub fn has(&self, feature: Feature) -> bool {
        self.features.contains(&feature)
    }
    /// Whether a person or an agent may be offered `operation`: it is listed, and it does not write on a read-only
    /// account. `can` still says the call exists (`send` is core), so ask this one before drawing a control or a tool.
    pub fn offers(&self, operation: Operation) -> bool {
        self.can(operation) && !(operation.writes() && self.has(Feature::ReadOnly))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    #[serde(rename = "ref")]
    pub reference: Ref,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    #[serde(rename = "ref")]
    pub reference: Ref,
    pub name: String,
    pub kind: ChannelKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unread: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Person {
    #[serde(rename = "ref")]
    pub reference: Ref,
    pub name: String,
    pub handle: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub is_bot: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

/// An emoji on a message: the short name without colons, how many reacted, and whether the signed-in person did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reaction {
    pub name: String,
    pub count: u32,
    /// Actor ids, when the provider says them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub by: Vec<String>,
    pub me: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    pub url: String,
    pub kind: AttachmentKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    #[serde(rename = "ref")]
    pub reference: Ref,
    pub channel: Ref,
    /// The root message, when this is a reply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Ref>,
    /// A markdown subset.
    pub text: String,
    pub author: Actor,
    /// Milliseconds since the epoch, UTC.
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<i64>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    /// The replies under a root message; 0 for any other message.
    #[serde(default)]
    pub reply_count: u32,
    /// Set when an agent sent the message: `Sam's agent`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelQuery {
    /// Empty means every kind.
    #[serde(default)]
    pub kinds: Vec<ChannelKind>,
    /// A part of the name, case does not matter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default)]
    pub include_archived: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<Ref>,
    /// An actor id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NewMessage {
    pub channel: Ref,
    pub text: String,
    /// A reply goes in the thread of this message (its root, when it is itself a reply).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_thread_of: Option<Ref>,
}

impl NewMessage {
    pub fn to(channel: &Ref, text: &str) -> Self {
        Self {
            channel: channel.clone(),
            text: text.to_string(),
            in_thread_of: None,
        }
    }

    pub fn reply(root: &Ref, channel: &Ref, text: &str) -> Self {
        Self {
            in_thread_of: Some(root.clone()),
            ..Self::to(channel, text)
        }
    }
}

/// What to follow. No channel means all of them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    #[serde(default)]
    pub channels: Vec<Ref>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub kind: EventKind,
    pub channel: Ref,
    pub message: Ref,
    /// The message as it is now. Absent for `deleted`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Message>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reaction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<Actor>,
}

/// One entity in an export or import batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    #[serde(rename = "type")]
    pub kind: EntityKind,
    pub entity: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}
