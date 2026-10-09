use serde::{Deserialize, Serialize};

/// What a channel is. A dm has two people; a group dm has more, and no name of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    Public,
    Private,
    Dm,
    GroupDm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    File,
    Image,
    Link,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Posted,
    Edited,
    Deleted,
    ReactionAdded,
    ReactionRemoved,
}

/// How much of the markdown subset of the spec the service keeps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Formatting {
    /// None: the screen shows the text as typed.
    #[default]
    Plain,
    /// Bold, italic, code and links.
    Basic,
    /// All of the subset.
    Rich,
}

/// A call a messaging provider may offer. The core ones every provider has; the rest are optional.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Channels,
    History,
    Thread,
    Send,
    Subscribe,
    Search,
    Edit,
    Delete,
    React,
    MarkRead,
    Person,
    Export,
    Import,
}

/// Something a provider's service has, which changes what the screen may show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    Threads,
    Reactions,
    Edits,
    Attachments,
    Presence,
    Typing,
    /// The account may not change the service. It may list `send` (it is core) and the other write calls, and every
    /// one of them answers `Provider { code: "read_only" }` without reaching the service. The screen and the agent
    /// tools hide what writes.
    ReadOnly,
}

impl Operation {
    /// Whether the call changes the service, so a read-only account does not offer it.
    pub fn writes(self) -> bool {
        matches!(
            self,
            Operation::Send
                | Operation::Edit
                | Operation::Delete
                | Operation::React
                | Operation::MarkRead
                | Operation::Import
        )
    }
}
/// The entity kinds of an export or import batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Channel,
    Message,
    Person,
    Workspace,
}
