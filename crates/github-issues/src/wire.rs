//! The parts of GitHub's JSON that the provider reads. Anything else stays in the task's `raw`.
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct User {
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Label {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    pub state: String,
    #[serde(default)]
    pub state_reason: Option<String>,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub assignees: Vec<User>,
    pub created_at: String,
    pub updated_at: String,
    /// Present on a pull request, which is not a task.
    #[serde(default)]
    pub pull_request: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Comment {
    pub id: u64,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub body: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Rename {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SourceIssue {
    pub number: u64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub pull_request: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Source {
    #[serde(default)]
    pub issue: Option<SourceIssue>,
}

/// One entry of an issue's timeline. Which fields are there depends on `event`.
#[derive(Clone, Debug, Deserialize)]
pub struct TimelineEvent {
    pub event: String,
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub created_at: Option<String>,
    /// Who did it, for most events.
    #[serde(default)]
    pub actor: Option<User>,
    /// Who wrote it, for a comment.
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub label: Option<Label>,
    #[serde(default)]
    pub assignee: Option<User>,
    #[serde(default)]
    pub state_reason: Option<String>,
    #[serde(default)]
    pub rename: Option<Rename>,
    #[serde(default)]
    pub commit_id: Option<String>,
    #[serde(default)]
    pub source: Option<Source>,
}
