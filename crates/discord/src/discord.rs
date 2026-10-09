use std::{
    collections::HashMap,
    sync::{Arc, mpsc::channel},
    thread,
};

use atelier_capabilities::{
    Actor, AuthKind, CapError, CapResult, Limits, Ref, Subscription,
    messaging::{
        Channel, ChannelQuery, Event, Feature, Filter, Formatting, Message, MessagingCapabilities,
        MessagingProvider, NewMessage, Operation, Page, SearchQuery, Workspace, workspace_ref,
    },
};

use crate::{
    DiscordConfig, Runner,
    core::{Core, PAGE_MAX, limit_of},
    helpers::{DM, is_snowflake},
    poll::poll,
    wire::SearchView,
};

fn offset_of(cursor: Option<&str>) -> CapResult<usize> {
    match cursor {
        None => Ok(0),
        Some(c) => c.parse().map_err(|_| CapError::invalid("cursor")),
    }
}

/// The Discord provider of the messaging capability. It runs `discordcli` for every call and keeps no copy of Discord,
/// so it needs no login of its own: the login is the one `discordcli` has.
///
/// It reads by default. `send` runs only when [`DiscordConfig::allow_writes`] is `true`, because the login is a user
/// token (see `docs/capabilities/discord-notes.md`). It lists no `edit`, `delete`, `react`, `mark_read` or `person`:
/// `discordcli` has no command for them.
pub struct DiscordMessaging {
    core: Arc<Core>,
}

impl DiscordMessaging {
    pub fn new(runner: impl Runner + 'static, config: DiscordConfig) -> Self {
        Self {
            core: Arc::new(Core::new(Box::new(runner), config)),
        }
    }
}

impl MessagingProvider for DiscordMessaging {
    fn provider(&self) -> &str {
        "discord"
    }

    fn account(&self) -> &str {
        &self.core.config.account
    }

    fn capabilities(&self) -> MessagingCapabilities {
        let mut operations = MessagingCapabilities::CORE.to_vec();
        if self.core.can_search() {
            operations.push(Operation::Search);
        }
        MessagingCapabilities {
            operations,
            // A reply is a thread here (see `thread`). Discord keeps no reaction or edit time in what the tool prints.
            features: vec![Feature::Threads, Feature::Attachments],
            // Discord shows bold, italic, strike, code, quotes and lists, but not `[text](url)` from a person.
            formatting: Formatting::Basic,
            limits: Limits {
                page_max: Some(PAGE_MAX as u32),
                per_minute: None,
            },
            auth: vec![AuthKind::BrowserSession],
        }
    }

    fn whoami(&self) -> CapResult<Actor> {
        self.core.me()
    }

    fn workspace(&self) -> CapResult<Workspace> {
        let reference = workspace_ref(&self.core.ref_in(&self.core.config.account, ""));
        let Some(guild) = self.core.guild() else {
            return Ok(Workspace {
                reference,
                name: "Direct messages".into(),
                raw: None,
            });
        };
        let row = self.core.server_row(guild)?;
        Ok(Workspace {
            reference,
            name: row
                .as_ref()
                .map_or_else(|| guild.to_string(), |r| r.name.clone()),
            raw: row.and_then(|r| serde_json::to_value(r).ok()),
        })
    }

    fn channels(&self, query: &ChannelQuery) -> CapResult<Page<Channel>> {
        let text = query.text.as_deref().map(str::to_lowercase);
        let all: Vec<Channel> = self
            .core
            .all_channels()?
            .into_iter()
            .filter(|c| query.kinds.is_empty() || query.kinds.contains(&c.kind))
            .filter(|c| {
                text.as_ref()
                    .is_none_or(|t| c.name.to_lowercase().contains(t))
            })
            .collect();
        let start = offset_of(query.cursor.as_deref())?;
        let limit = limit_of(query.limit);
        let items: Vec<Channel> = all.iter().skip(start).take(limit).cloned().collect();
        let next_cursor =
            (start + items.len() < all.len()).then(|| (start + items.len()).to_string());
        Ok(Page { items, next_cursor })
    }

    /// A page of the newest messages that answer none. The cursor is the id of the oldest message the last page read, so
    /// a page can hold fewer messages than asked, or none, when replies filled it. A root counts only the replies in its
    /// own page.
    fn history(
        &self,
        channel: &Ref,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> CapResult<Page<Message>> {
        let id = self.core.channel_id(channel)?;
        if cursor.is_some_and(|c| !is_snowflake(c)) {
            return Err(CapError::invalid("cursor"));
        }
        let view = self.core.read_rows(id, limit_of(limit), cursor, None)?;
        let mut items: Vec<Message> = self
            .core
            .messages_of(channel, &view.rows)
            .into_iter()
            .filter(|m| m.parent.is_none())
            .collect();
        items.reverse();
        let next_cursor = view
            .has_more
            .then(|| view.rows.first().map(|r| r.id.clone()))
            .flatten();
        Ok(Page { items, next_cursor })
    }

    /// The root and the replies in the channel that answer it, or answer one of its replies, oldest first. Discord has no
    /// list of replies, so this reads up to 500 messages after the root. A thread channel of Discord is a channel: read
    /// it with `history`.
    fn thread(&self, root: &Ref, _cursor: Option<&str>) -> CapResult<Page<Message>> {
        Ok(Page {
            items: self.core.thread(root)?,
            next_cursor: None,
        })
    }

    fn send(&self, new: &NewMessage, by: &Actor) -> CapResult<Message> {
        self.core
            .send(&new.channel, &new.text, new.in_thread_of.as_ref(), by)
    }

    /// Looks for new messages every `poll_every`, in each channel asked for, replies included. It sees no edit, delete or
    /// reaction. An empty filter means every channel the provider lists, and more than `max_polled` is refused.
    fn subscribe(&self, filter: &Filter) -> CapResult<Subscription<Event>> {
        let channels: Vec<Ref> = if filter.channels.is_empty() {
            self.core
                .all_channels()?
                .into_iter()
                .map(|c| c.reference)
                .collect()
        } else {
            filter.channels.clone()
        };
        if channels.len() > self.core.config.max_polled {
            return Err(CapError::invalid("filter"));
        }
        let mut last: HashMap<Ref, Option<String>> = HashMap::new();
        for channel in &channels {
            let id = self.core.channel_id(channel)?;
            let view = self.core.read_rows(id, 1, None, None)?;
            last.insert(channel.clone(), view.rows.last().map(|r| r.id.clone()));
        }
        let (tx, rx) = channel();
        let (subscription, stop) = Subscription::new(rx);
        let core = self.core.clone();
        thread::spawn(move || poll(core, channels, last, tx, stop));
        Ok(subscription)
    }

    /// Searches the one server of the account. A word that is not an author id finds nobody, so it gives an empty page
    /// without a command.
    fn search(&self, query: &SearchQuery) -> CapResult<Page<Message>> {
        let Some(guild) = self.core.guild().filter(|_| self.core.can_search()) else {
            return Err(CapError::unsupported("search messages"));
        };
        let text = query.text.trim();
        if text.is_empty() {
            return Err(CapError::invalid("text"));
        }
        if query.from.as_deref().is_some_and(|f| !is_snowflake(f)) {
            return Ok(Page {
                items: Vec::new(),
                next_cursor: None,
            });
        }
        let channel = match &query.channel {
            Some(channel) if channel.account == DM => return Err(CapError::invalid("channel")),
            Some(channel) => Some(self.core.channel_id(channel)?.to_string()),
            None => None,
        };
        let offset = offset_of(query.cursor.as_deref())?.to_string();
        let limit = limit_of(query.limit).to_string();
        let mut args = vec![
            "search", "--json", "-n", &limit, "--offset", &offset, "--server", guild,
        ];
        if let Some(channel) = &channel {
            args.extend(["--channel", channel]);
        }
        if let Some(from) = &query.from {
            args.extend(["--from", from]);
        }
        args.extend(["--", text]);
        let view: SearchView = self.core.call(&args)?;
        let mut items = Vec::new();
        for row in &view.rows {
            let channel = self.core.ref_in(&self.core.config.account, &row.channel_id);
            items.extend(self.core.messages_of(&channel, std::slice::from_ref(row)));
        }
        let next_cursor = view
            .next_offset
            .filter(|_| view.has_more)
            .map(|n| n.to_string());
        Ok(Page { items, next_cursor })
    }
}
