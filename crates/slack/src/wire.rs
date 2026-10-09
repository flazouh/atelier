//! What `slackcli --json` prints. The shapes are the view types of the slackcli source (`src/services/slack.ts` and
//! `src/domain/rows.ts`). A key that is `undefined` there is absent here, so those fields are options.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRow {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub filetype: Option<String>,
    #[serde(default)]
    pub mimetype: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub permalink: Option<String>,
}

/// A row of `read` and `thread`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageRow {
    pub author: String,
    pub message: String,
    pub at: String,
    pub url: String,
    #[serde(default)]
    pub replies: Option<u32>,
    #[serde(default)]
    pub files: Vec<FileRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadView {
    pub channel: String,
    pub rows: Vec<MessageRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadView {
    pub channel: String,
    #[serde(default)]
    pub total: u32,
    #[serde(default)]
    pub has_more: bool,
    pub rows: Vec<MessageRow>,
}

/// A row of `search`. The link is absent for a result Slack gave no permalink for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchRow {
    pub channel: String,
    pub author: String,
    pub message: String,
    pub at: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub files: Vec<FileRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchView {
    #[serde(default)]
    pub total: Option<u64>,
    pub rows: Vec<SearchRow>,
}

/// What `send`, `reply` and `edit` print.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Posted {
    pub channel: String,
    pub ts: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub user: String,
    pub user_id: String,
    pub team_id: String,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub credential: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRow {
    pub id: String,
    #[serde(default)]
    pub mention: Option<String>,
    pub handle: String,
    #[serde(default)]
    pub real_name: Option<String>,
    #[serde(default)]
    pub bot: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsersView {
    pub rows: Vec<UserRow>,
}
