//! What `discordcli --json` prints. The shapes are the results of `src/discord.mjs` and `src/rows.mjs` of discordcli. An
//! id is always a string there, so every id is a `String` here. A key that can be `null` is an option.
use serde::{Deserialize, Serialize};

/// What `whoami` prints.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub id: String,
    pub username: String,
    pub name: String,
    #[serde(default)]
    pub credential_source: Option<String>,
}

/// A row of `servers`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerRow {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub owner: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServersView {
    pub rows: Vec<ServerRow>,
    #[serde(default)]
    pub has_more: bool,
    /// The cursor for the next page: pass it as `--after`.
    #[serde(default)]
    pub after: Option<String>,
}

/// A row of `channels` and `dms`. `kind` is Discord's channel type number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelRow {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type")]
    pub kind: u32,
    #[serde(default)]
    pub guild_id: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelsView {
    pub rows: Vec<ChannelRow>,
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub has_more: bool,
    #[serde(default)]
    pub next_offset: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentRow {
    pub id: String,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub content_type: Option<String>,
}

/// A row of `read`, `thread` and `search`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRow {
    pub id: String,
    pub channel_id: String,
    #[serde(default)]
    pub guild_id: Option<String>,
    #[serde(default)]
    pub author_id: Option<String>,
    pub author: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub attachments: Vec<AttachmentRow>,
    /// The id of the message this one answers.
    #[serde(default)]
    pub reply_to: Option<String>,
}

/// What `read` prints: the rows are oldest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadView {
    pub rows: Vec<MessageRow>,
    #[serde(default)]
    pub has_more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchView {
    pub rows: Vec<MessageRow>,
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub has_more: bool,
    #[serde(default)]
    pub next_offset: Option<u64>,
}

/// What `send` and `reply` print.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Posted {
    pub id: String,
    pub channel_id: String,
    #[serde(default)]
    pub url: Option<String>,
}
