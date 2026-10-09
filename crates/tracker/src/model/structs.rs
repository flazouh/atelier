use serde::{Deserialize, Serialize};

use super::types::{ActivityKind, Assignee, Priority, Status};

/// A task's id in its tracker. Opaque: the local tracker uses a number, Linear a UUID, GitHub a node id.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for TaskId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

/// A session working on a task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLink {
    pub session_id: String,
    pub title: String,
    /// The agent's name: "Claude".
    pub agent: String,
}

/// A pull request that belongs to a task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrLink {
    pub number: u64,
    /// `owner/repo`.
    pub repo: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    /// The short id people say: "LAT-42".
    pub key: String,
    pub title: String,
    /// Markdown.
    pub description: String,
    pub status: Status,
    pub priority: Priority,
    pub assignee: Option<Assignee>,
    /// Sorted.
    pub labels: Vec<String>,
    pub project: Option<String>,
    /// The task this is a sub-task of.
    pub parent: Option<TaskId>,
    pub sessions: Vec<SessionLink>,
    pub prs: Vec<PrLink>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// What a new task is made from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NewTask {
    pub title: String,
    pub description: String,
    pub status: Status,
    pub priority: Priority,
    pub assignee: Option<Assignee>,
    pub labels: Vec<String>,
    pub project: Option<String>,
    pub parent: Option<TaskId>,
}

impl NewTask {
    pub fn titled(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: String::new(),
            status: Status::Todo,
            priority: Priority::None,
            assignee: None,
            labels: Vec::new(),
            project: None,
            parent: None,
        }
    }
}

/// A change to fields. A field left `None` stays as it is; an `Option<Option<_>>` field can also be taken
/// off with `Some(None)`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<Status>,
    pub priority: Option<Priority>,
    pub assignee: Option<Option<Assignee>>,
    pub add_labels: Vec<String>,
    pub remove_labels: Vec<String>,
    pub project: Option<Option<String>>,
    pub parent: Option<Option<TaskId>>,
}

impl Patch {
    pub fn status(status: Status) -> Self {
        Self {
            status: Some(status),
            ..Self::default()
        }
    }
}

/// Which tasks to list. Every part that is set must match.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Query {
    /// Any of these statuses. Empty means all.
    pub statuses: Vec<Status>,
    /// Assigned to this person or agent, by name.
    pub assignee: Option<String>,
    pub label: Option<String>,
    pub priority: Option<Priority>,
    /// Words the title or the key holds, whatever the case.
    pub text: Option<String>,
    /// Only the sub-tasks of this task.
    pub parent: Option<TaskId>,
}

/// One line of the append-only log. `by` is a name, or `rule:<id>` for the automation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    pub id: i64,
    pub task: TaskId,
    pub at: i64,
    pub by: String,
    pub kind: ActivityKind,
}
