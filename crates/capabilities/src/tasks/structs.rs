use serde::{Deserialize, Serialize};

use super::types::{ActivityKind, Category, Change, EntityKind, EventKind, LinkKind, Priority, Sort};
use crate::{Actor, Ref};

/// A status as a team names it, and the category it belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub id: String,
    pub name: String,
    pub category: Category,
}

impl Status {
    /// The status of a provider with no custom states: the id, the name and the category are the same word.
    pub fn plain(category: Category) -> Self {
        let name = match category {
            Category::Backlog => "backlog",
            Category::Todo => "todo",
            Category::InProgress => "in_progress",
            Category::InReview => "in_review",
            Category::Done => "done",
            Category::Canceled => "canceled",
        };
        Self { id: name.into(), name: name.into(), category }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLink {
    pub kind: LinkKind,
    /// A reference for a thing in a capability; the address for a `url`.
    #[serde(rename = "ref")]
    pub reference: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Task {
    #[serde(rename = "ref")]
    pub reference: Ref,
    /// The human id: `ENG-123`, `#42`.
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub status: Status,
    pub priority: Priority,
    #[serde(default)]
    pub assignees: Vec<Actor>,
    #[serde(default)]
    pub labels: Vec<Ref>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<Ref>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Ref>,
    #[serde(default)]
    pub links: Vec<TaskLink>,
    /// Milliseconds since the epoch, UTC.
    pub created_at: i64,
    pub updated_at: i64,
    /// Opaque. Sent back on update, so a stale write is refused.
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<f64>,
    /// The provider's own JSON, kept for export and import.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    #[serde(rename = "ref")]
    pub reference: Ref,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    #[serde(rename = "ref")]
    pub reference: Ref,
    pub key: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    #[serde(rename = "ref")]
    pub reference: Ref,
    pub task: Ref,
    pub author: Actor,
    pub body: String,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    #[serde(rename = "ref")]
    pub reference: Ref,
    pub task: Ref,
    pub at: i64,
    pub by: Actor,
    pub kind: ActivityKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Query {
    #[serde(default)]
    pub status: Vec<Category>,
    /// An actor id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<Ref>,
    #[serde(default)]
    pub labels: Vec<Ref>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Tasks that link to this thing: a session, a pull request, a message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_to: Option<String>,
    #[serde(default)]
    pub sort: Sort,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NewTask {
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// A status id of the provider. Empty takes the provider's first `todo` or `backlog` state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<Ref>,
    #[serde(default)]
    pub labels: Vec<Ref>,
    /// Actor ids.
    #[serde(default)]
    pub assignees: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Ref>,
}

impl NewTask {
    pub fn titled(title: impl Into<String>) -> Self {
        Self { title: title.into(), ..Self::default() }
    }
}

/// The fields to change. A field that is absent stays; a field set to `null` clears.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Change::is_keep")]
    pub description: Change<String>,
    /// A status id of the provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    #[serde(default, skip_serializing_if = "Change::is_keep")]
    pub project: Change<Ref>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<Ref>>,
    /// Actor ids. An empty list clears.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignees: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Change::is_keep")]
    pub parent: Change<Ref>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub kind: EventKind,
    pub task: Task,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<Activity>,
}

/// One entity in an export or import batch, with the provider's own JSON beside it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    #[serde(rename = "type")]
    pub kind: EntityKind,
    pub entity: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}
