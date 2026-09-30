//! The data of a tracker, in plain types. Times are seconds since the Unix epoch. Nothing here knows a UI
//! type: the app maps a [`Task`] to what it shows.
use serde::{Deserialize, Serialize};

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

/// Where a task stands. The order of the variants is the flow of the board.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Status {
    Backlog,
    Todo,
    InProgress,
    InReview,
    Done,
    Canceled,
}

impl Status {
    pub const ALL: [Status; 6] =
        [Self::Backlog, Self::Todo, Self::InProgress, Self::InReview, Self::Done, Self::Canceled];

    /// The word stored and sent: "in_progress".
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::InReview => "in_review",
            Self::Done => "done",
            Self::Canceled => "canceled",
        }
    }

    pub fn parse(word: &str) -> Option<Status> {
        Self::ALL.into_iter().find(|s| s.as_str() == word)
    }

    /// The task is finished, one way or the other.
    pub fn is_closed(self) -> bool {
        matches!(self, Self::Done | Self::Canceled)
    }
}

/// How urgent. The number is the one Linear uses (0 none, 1 urgent, 2 high, 3 medium, 4 low), and the one
/// stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Priority {
    #[default]
    None,
    Urgent,
    High,
    Medium,
    Low,
}

impl Priority {
    pub const ALL: [Priority; 5] = [Self::None, Self::Urgent, Self::High, Self::Medium, Self::Low];

    pub fn number(self) -> i64 {
        match self {
            Self::None => 0,
            Self::Urgent => 1,
            Self::High => 2,
            Self::Medium => 3,
            Self::Low => 4,
        }
    }

    pub fn from_number(n: i64) -> Priority {
        Self::ALL.into_iter().find(|p| p.number() == n).unwrap_or_default()
    }
}

/// Who a task is assigned to. The name is the id: the app maps an agent's name to its look.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Assignee {
    Person(String),
    Agent(String),
}

impl Assignee {
    pub fn name(&self) -> &str {
        match self {
            Self::Person(name) | Self::Agent(name) => name,
        }
    }

    pub fn is_agent(&self) -> bool {
        matches!(self, Self::Agent(_))
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
        Self { status: Some(status), ..Self::default() }
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

/// What to append to a task's activity. A session and a pull request also become links of the task.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Entry {
    Comment(String),
    SessionStarted(SessionLink),
    PrOpened(PrLink),
    PrMerged(PrLink),
    /// A commit the session made. It links nothing.
    Commit { sha: String, subject: String },
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ActivityKind {
    Created,
    StatusChanged { from: Status, to: Status },
    Assigned { to: Option<Assignee> },
    /// Any other field: "title", "description", "priority", "labels", "project", "parent".
    Edited { field: String },
    Commented { text: String },
    SessionStarted { session: SessionLink },
    PrOpened { pr: PrLink },
    PrMerged { pr: PrLink },
    Commit { sha: String, subject: String },
}

/// A change, told to whoever subscribed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    Created(Task),
    Updated(Task),
    Activity(Activity),
}
