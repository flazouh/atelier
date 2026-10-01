use serde::{Deserialize, Serialize};

use super::structs::{Activity, PrLink, SessionLink, Task};

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
