//! The work behind [`DiscordMessaging`](crate::DiscordMessaging): the commands, the mapping of their rows, and the
//! checks on what goes in. It holds no copy of Discord.
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use atelier_capabilities::{
    Actor, CapError, CapResult, Ref,
    messaging::{
        Attachment, AttachmentKind, Channel, Message, message_ref, origin_of, require_message,
    },
};
use serde::de::DeserializeOwned;

use crate::{
    DiscordConfig, Runner,
    helpers::{DM, error_of, is_snowflake, kind_of, link_of, snowflake_ms, utf16_len},
    text::{from_discord, to_discord},
    types::RunError,
    wire::{
        AttachmentRow, ChannelRow, ChannelsView, Identity, MessageRow, Posted, ReadView, ServerRow,
        ServersView,
    },
};

/// Discord refuses a message of more than this many UTF-16 units.
const MAX_TEXT: usize = 2000;
/// How far to follow a chain of replies up to the message that began it.
const MAX_HOPS: usize = 8;
/// How many pages of 100 messages a thread reads after its root.
const THREAD_PAGES: usize = 5;
/// How many pages of 100 channels a list reads.
const LIST_PAGES: usize = 20;
pub const PAGE_MAX: usize = 100;
const PAGE_DEFAULT: usize = 50;

pub fn limit_of(limit: Option<u32>) -> usize {
    limit
        .map_or(PAGE_DEFAULT, |n| n as usize)
        .clamp(1, PAGE_MAX)
}

pub struct Core {
    runner: Box<dyn Runner>,
    pub config: DiscordConfig,
    me: Mutex<Option<Identity>>,
}

impl Core {
    pub fn new(runner: Box<dyn Runner>, config: DiscordConfig) -> Self {
        Self {
            runner,
            config,
            me: Mutex::new(None),
        }
    }

    pub fn call<T: DeserializeOwned>(&self, args: &[&str]) -> CapResult<T> {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let out = self.runner.run(&args).map_err(|e| match e {
            RunError::NotInstalled(program) => CapError::Provider {
                code: "discordcli_missing".into(),
                message: format!("`{program}` is not installed"),
            },
            RunError::Io(message) => CapError::Provider {
                code: "io".into(),
                message,
            },
        })?;
        if out.code != 0 {
            return Err(error_of(&out.stderr));
        }
        serde_json::from_str(&out.stdout).map_err(|e| CapError::Provider {
            code: "bad_output".into(),
            message: format!("discordcli printed something unexpected: {e}"),
        })
    }

    pub fn identity(&self) -> CapResult<Identity> {
        if let Some(known) = self.me.lock().ok().and_then(|m| m.clone()) {
            return Ok(known);
        }
        let identity: Identity = self.call(&["whoami", "--json"])?;
        if let Ok(mut me) = self.me.lock() {
            *me = Some(identity.clone());
        }
        Ok(identity)
    }

    pub fn me(&self) -> CapResult<Actor> {
        let id = self.identity()?;
        Ok(Actor::person(id.id, id.name))
    }

    /// The server this provider serves, or `None` when it serves the direct messages only.
    pub fn guild(&self) -> Option<&str> {
        (self.config.account != DM).then_some(self.config.account.as_str())
    }

    pub fn serves_dms(&self) -> bool {
        self.config.include_dms || self.config.account == DM
    }

    /// Search runs in one server, so a provider that also holds direct messages cannot offer it for a channel there.
    pub fn can_search(&self) -> bool {
        self.guild().is_some() && !self.config.include_dms
    }

    pub fn ref_in(&self, account: &str, id: &str) -> Ref {
        Ref {
            capability: "messaging".into(),
            provider: "discord".into(),
            account: account.to_string(),
            id: id.to_string(),
        }
    }

    /// The reference of a channel from its row: a channel of a server is in the account of the server.
    fn channel_ref_of(&self, row: &ChannelRow) -> Ref {
        let account = if row.guild_id.is_some() {
            &self.config.account
        } else {
            DM
        };
        self.ref_in(account, &row.id)
    }

    fn own(&self, reference: &Ref, field: &str) -> CapResult<()> {
        let account_ok = reference.account == self.config.account
            || (reference.account == DM && self.serves_dms());
        if reference.capability != "messaging" || reference.provider != "discord" || !account_ok {
            return Err(CapError::invalid(field));
        }
        Ok(())
    }

    /// The Discord id of the channel that `channel` names. A channel id that cannot be an id does not exist.
    pub fn channel_id<'a>(&self, channel: &'a Ref) -> CapResult<&'a str> {
        self.own(channel, "channel")?;
        if channel.id.is_empty()
            || channel.id.contains(':')
            || channel.id.starts_with("user/")
            || channel.id == "workspace"
        {
            return Err(CapError::invalid("channel"));
        }
        if !is_snowflake(&channel.id) {
            return Err(CapError::not_found("the channel"));
        }
        Ok(&channel.id)
    }

    /// The channel and the id of the message that `message` names.
    pub fn message_target(&self, message: &Ref, field: &str) -> CapResult<(Ref, String)> {
        self.own(message, field)?;
        let (channel, id) = require_message(message, field)?;
        self.channel_id(&channel)?;
        if !is_snowflake(&id) {
            return Err(CapError::not_found("the message"));
        }
        Ok((channel, id))
    }

    fn attachments(files: &[AttachmentRow]) -> Vec<Attachment> {
        files
            .iter()
            .map(|f| Attachment {
                id: f.id.clone(),
                name: f.filename.clone().unwrap_or_else(|| f.id.clone()),
                mime: f.content_type.clone(),
                size: f.size,
                url: f.url.clone().unwrap_or_default(),
                kind: if f
                    .content_type
                    .as_deref()
                    .is_some_and(|m| m.starts_with("image/"))
                {
                    AttachmentKind::Image
                } else {
                    AttachmentKind::File
                },
            })
            .collect()
    }

    /// Maps rows of one channel, oldest first, to messages, oldest first. A reply gets the root of its chain as `parent`
    /// when the chain is in these rows, and the message it answers when it is not. A root counts the replies in these
    /// rows, which are all the rows it can see.
    pub fn messages_of(&self, channel: &Ref, rows: &[MessageRow]) -> Vec<Message> {
        let mut roots: HashMap<&str, &str> = HashMap::new();
        let mut counts: HashMap<&str, u32> = HashMap::new();
        for row in rows {
            if let Some(target) = row.reply_to.as_deref() {
                let root = roots.get(target).copied().unwrap_or(target);
                roots.insert(&row.id, root);
                *counts.entry(root).or_default() += 1;
            }
        }
        rows.iter()
            .filter_map(|row| {
                let parent = roots.get(row.id.as_str()).map(|r| message_ref(channel, r));
                self.message_from(channel, row, parent, counts.get(row.id.as_str()).copied())
            })
            .collect()
    }

    fn message_from(
        &self,
        channel: &Ref,
        row: &MessageRow,
        parent: Option<Ref>,
        reply_count: Option<u32>,
    ) -> Option<Message> {
        Some(Message {
            reference: message_ref(channel, &row.id),
            channel: channel.clone(),
            parent,
            text: from_discord(&row.message, &channel.account),
            author: Actor::person(
                row.author_id.clone().unwrap_or_else(|| row.author.clone()),
                row.author.clone(),
            ),
            created_at: snowflake_ms(&row.id)?,
            edited_at: None,
            attachments: Self::attachments(&row.attachments),
            reactions: Vec::new(),
            reply_count: reply_count.unwrap_or(0),
            origin: None,
            raw: serde_json::to_value(row).ok(),
        })
    }

    /// Every channel the provider serves that holds messages, the server's first and then the direct messages.
    pub fn all_channels(&self) -> CapResult<Vec<Channel>> {
        let mut rows: Vec<ChannelRow> = Vec::new();
        if let Some(guild) = self.guild() {
            rows.extend(self.list_pages(|offset| {
                self.call::<ChannelsView>(&[
                    "channels", "--json", "-n", "100", "--offset", offset, guild,
                ])
            })?);
        }
        if self.serves_dms() {
            rows.extend(self.list_pages(|offset| {
                self.call::<ChannelsView>(&["dms", "--json", "-n", "100", "--offset", offset])
            })?);
        }
        Ok(rows
            .iter()
            .filter_map(|row| {
                Some(Channel {
                    reference: self.channel_ref_of(row),
                    name: row.name.clone(),
                    kind: kind_of(row.kind)?,
                    topic: None,
                    archived: false,
                    member_count: None,
                    unread: None,
                    created_at: snowflake_ms(&row.id),
                    raw: serde_json::to_value(row).ok(),
                })
            })
            .collect())
    }

    fn list_pages(
        &self,
        mut page: impl FnMut(&str) -> CapResult<ChannelsView>,
    ) -> CapResult<Vec<ChannelRow>> {
        let mut rows = Vec::new();
        let mut offset = 0;
        for _ in 0..LIST_PAGES {
            let view = page(&offset.to_string())?;
            rows.extend(view.rows);
            match view.next_offset {
                Some(next) if view.has_more => offset = next,
                _ => break,
            }
        }
        Ok(rows)
    }

    /// The server's row, to name the workspace.
    pub fn server_row(&self, guild: &str) -> CapResult<Option<ServerRow>> {
        let mut after: Option<String> = None;
        for _ in 0..LIST_PAGES {
            let view: ServersView = match &after {
                None => self.call(&["servers", "--json", "-n", "100"])?,
                Some(cursor) => {
                    self.call(&["servers", "--json", "-n", "100", "--after", cursor])?
                }
            };
            if let Some(found) = view.rows.into_iter().find(|s| s.id == guild) {
                return Ok(Some(found));
            }
            match view.after {
                Some(next) if view.has_more => after = Some(next),
                _ => break,
            }
        }
        Ok(None)
    }

    /// One page of raw rows, oldest first, with the cursor of the page before it.
    pub fn read_rows(
        &self,
        channel_id: &str,
        limit: usize,
        before: Option<&str>,
        after: Option<&str>,
    ) -> CapResult<ReadView> {
        let n = limit.to_string();
        let mut args = vec!["read", "--json", "-n", &n];
        if let Some(before) = before {
            args.extend(["--before", before]);
        }
        if let Some(after) = after {
            args.extend(["--after", after]);
        }
        args.push(channel_id);
        self.call(&args)
    }

    /// One message, by its link. `read` with a link gives the messages around it, and one is the message.
    pub fn fetch_message(&self, channel: &Ref, id: &str) -> CapResult<MessageRow> {
        let link = link_of(channel, id);
        let view: ReadView = self.call(&["read", "--json", "-n", "1", &link])?;
        view.rows
            .into_iter()
            .find(|row| row.id == id)
            .ok_or_else(|| CapError::not_found("the message"))
    }

    /// The message that began the chain of replies `id` is in. A reply whose original is gone is taken as a root.
    pub fn root_of(&self, channel: &Ref, id: &str) -> CapResult<MessageRow> {
        let mut row = self.fetch_message(channel, id)?;
        for _ in 0..MAX_HOPS {
            let Some(target) = row.reply_to.clone() else {
                break;
            };
            match self.fetch_message(channel, &target) {
                Ok(next) => row = next,
                Err(CapError::NotFound { .. }) => break,
                Err(other) => return Err(other),
            }
        }
        Ok(row)
    }

    pub fn require_writes(&self) -> CapResult<()> {
        if self.config.allow_writes {
            return Ok(());
        }
        Err(CapError::Provider {
            code: "read_only".into(),
            message:
                "this Discord provider is read-only; a user-token login may not send messages, \
                      so sending needs allow_writes"
                    .into(),
        })
    }

    /// The root and its replies. Discord has no thread under a message, only replies in the channel, so the replies are
    /// the messages after the root that answer it or answer one of its replies. It reads at most
    /// `THREAD_PAGES * 100` messages after the root.
    pub fn thread(&self, message: &Ref) -> CapResult<Vec<Message>> {
        let (channel, id) = self.message_target(message, "message")?;
        let root = self.root_of(&channel, &id)?;
        let mut members: HashSet<String> = HashSet::from([root.id.clone()]);
        let mut replies: Vec<MessageRow> = Vec::new();
        let mut after = root.id.clone();
        for _ in 0..THREAD_PAGES {
            let view = self.read_rows(&channel.id, PAGE_MAX, None, Some(&after))?;
            for row in &view.rows {
                if row.reply_to.as_ref().is_some_and(|t| members.contains(t)) {
                    members.insert(row.id.clone());
                    replies.push(row.clone());
                }
            }
            match view.rows.last() {
                Some(last) if view.has_more => after = last.id.clone(),
                _ => break,
            }
        }
        let root_ref = message_ref(&channel, &root.id);
        let mut items = Vec::new();
        items.extend(self.message_from(&channel, &root, None, Some(replies.len() as u32)));
        items.extend(
            replies
                .iter()
                .filter_map(|row| self.message_from(&channel, row, Some(root_ref.clone()), None)),
        );
        Ok(items)
    }

    /// Sends a text, or replies to a message, as the signed-in person. Nothing runs unless writes are allowed.
    pub fn send(
        &self,
        channel: &Ref,
        text: &str,
        in_thread_of: Option<&Ref>,
        by: &Actor,
    ) -> CapResult<Message> {
        self.require_writes()?;
        let channel_id = self.channel_id(channel)?.to_string();
        if text.trim().is_empty() {
            return Err(CapError::invalid("text"));
        }
        let me = self.me()?;
        let origin = origin_of(by, &me.name);
        let mut body = to_discord(text);
        if let Some(origin) = &origin {
            body.push_str(&format!("\n\n_sent by {}_", to_discord(origin)));
        }
        if utf16_len(&body) > MAX_TEXT {
            return Err(CapError::invalid("text"));
        }
        let (posted, parent): (Posted, Option<Ref>) = match in_thread_of {
            None => (
                self.call(&["send", "--yes", "--json", "--", &channel_id, &body])?,
                None,
            ),
            Some(target) => {
                let (target_channel, id) = self.message_target(target, "in_thread_of")?;
                if target_channel != *channel {
                    return Err(CapError::invalid("in_thread_of"));
                }
                let root = self.root_of(channel, &id)?;
                let link = link_of(channel, &id);
                let posted = self.call(&["reply", "--yes", "--json", "--", &link, &body])?;
                (posted, Some(message_ref(channel, &root.id)))
            }
        };
        Ok(Message {
            reference: message_ref(channel, &posted.id),
            channel: channel.clone(),
            parent,
            text: text.to_string(),
            author: by.clone(),
            created_at: snowflake_ms(&posted.id).unwrap_or_default(),
            edited_at: None,
            attachments: Vec::new(),
            reactions: Vec::new(),
            reply_count: 0,
            origin,
            raw: serde_json::to_value(&posted).ok(),
        })
    }
}
