use std::{
    collections::HashSet,
    sync::{Arc, Mutex, mpsc::channel},
    thread,
    time::{Duration, Instant},
};

use atelier_capabilities::{
    Actor, AuthKind, CapError, CapResult, Limits, Ref, StopFlag, Subscription,
    messaging::{
        Attachment, AttachmentKind, Channel, ChannelQuery, Event, EventKind, Feature, Filter,
        Formatting, Message, MessagingCapabilities, MessagingProvider, NewMessage, Operation, Page,
        Person, SearchQuery, Workspace, message_ref, origin_of, person_ref, require_message,
        workspace_ref,
    },
};
use serde::de::DeserializeOwned;

use crate::{
    Runner, SlackConfig,
    helpers::{error_of, is_ts, now_ms, parse_permalink, to_mrkdwn, ts_to_ms},
    types::RunError,
    wire::{
        FileRow, Identity, MessageRow, Posted, ReadView, SearchView, ThreadView, UserRow, UsersView,
    },
};

const PAGE_DEFAULT: usize = 50;
const PAGE_MAX: usize = 100;

fn limit_of(limit: Option<u32>) -> usize {
    limit
        .map_or(PAGE_DEFAULT, |n| n as usize)
        .clamp(1, PAGE_MAX)
}

fn offset_of(cursor: Option<&str>) -> CapResult<usize> {
    match cursor {
        None => Ok(0),
        Some(c) => c.parse().map_err(|_| CapError::invalid("cursor")),
    }
}

/// The Slack provider of the messaging capability. It runs `slackcli` for every call and keeps no copy of Slack, so it
/// needs no login of its own: the login is the one `slackcli` has.
///
/// What `slackcli` cannot do, this provider does not list: `delete`, `react` and `mark_read`. It lists no channels of its
/// own either; it lists the ones in its [`SlackConfig`].
pub struct SlackMessaging {
    shared: Arc<Shared>,
}

struct Shared {
    runner: Box<dyn Runner>,
    config: SlackConfig,
    me: Mutex<Option<Identity>>,
}

impl SlackMessaging {
    pub fn new(runner: impl Runner + 'static, config: SlackConfig) -> Self {
        Self {
            shared: Arc::new(Shared {
                runner: Box::new(runner),
                config,
                me: Mutex::new(None),
            }),
        }
    }
}

impl Shared {
    fn call<T: DeserializeOwned>(&self, args: &[&str]) -> CapResult<T> {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let out = self.runner.run(&args).map_err(|e| match e {
            RunError::NotInstalled(program) => CapError::Provider {
                code: "slackcli_missing".into(),
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
            message: format!("slackcli printed something unexpected: {e}"),
        })
    }

    fn identity(&self) -> CapResult<Identity> {
        if let Some(known) = self.me.lock().ok().and_then(|m| m.clone()) {
            return Ok(known);
        }
        let identity: Identity = self.call(&["whoami", "--json"])?;
        if let Ok(mut me) = self.me.lock() {
            *me = Some(identity.clone());
        }
        Ok(identity)
    }

    fn me(&self) -> CapResult<Actor> {
        let id = self.identity()?;
        Ok(Actor::person(id.user_id, id.user))
    }

    fn account(&self) -> &str {
        &self.config.account
    }

    fn ref_of(&self, id: &str) -> Ref {
        Ref {
            capability: "messaging".into(),
            provider: "slack".into(),
            account: self.config.account.clone(),
            id: id.to_string(),
        }
    }

    fn own(&self, reference: &Ref, field: &str) -> CapResult<()> {
        if reference.capability != "messaging"
            || reference.provider != "slack"
            || reference.account != self.config.account
        {
            return Err(CapError::invalid(field));
        }
        Ok(())
    }

    /// The channel's Slack id, for a reference that names a channel.
    fn channel_id<'a>(&self, channel: &'a Ref) -> CapResult<&'a str> {
        self.own(channel, "channel")?;
        if channel.id.is_empty() || channel.id.contains(':') || channel.id.starts_with("user/") {
            return Err(CapError::invalid("channel"));
        }
        Ok(&channel.id)
    }

    /// The channel and the ts of a reference that names a message.
    fn message_target(&self, message: &Ref, field: &str) -> CapResult<(String, String)> {
        self.own(message, field)?;
        let (channel, ts) = require_message(message, field)?;
        if !is_ts(&ts) {
            return Err(CapError::invalid(field));
        }
        Ok((channel.id, ts))
    }

    fn author_of(&self, name: &str, me: &Identity) -> Actor {
        if name == me.user || name == me.user_id {
            Actor::person(me.user_id.clone(), me.user.clone())
        } else {
            Actor::person(name, name)
        }
    }

    fn attachments(files: &[FileRow]) -> Vec<Attachment> {
        files
            .iter()
            .map(|f| Attachment {
                id: f.id.clone(),
                name: f
                    .name
                    .clone()
                    .or_else(|| f.title.clone())
                    .unwrap_or_else(|| f.id.clone()),
                mime: f.mimetype.clone(),
                size: f.size,
                url: f.permalink.clone().unwrap_or_default(),
                kind: if f
                    .mimetype
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

    fn message_from(
        &self,
        row: &MessageRow,
        parent: Option<Ref>,
        me: &Identity,
    ) -> Option<Message> {
        let link = parse_permalink(&row.url)?;
        let channel = self.ref_of(&link.channel);
        Some(Message {
            reference: message_ref(&channel, &link.ts),
            channel,
            parent,
            text: row.message.clone(),
            author: self.author_of(&row.author, me),
            created_at: ts_to_ms(&link.ts)?,
            edited_at: None,
            attachments: Self::attachments(&row.files),
            reactions: Vec::new(),
            reply_count: row.replies.unwrap_or(0),
            origin: None,
            raw: serde_json::to_value(row).ok(),
        })
    }

    fn history(
        &self,
        channel: &Ref,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> CapResult<Page<Message>> {
        let id = self.channel_id(channel)?;
        let offset = offset_of(cursor)?;
        let limit = limit_of(limit);
        let want = (offset + limit + 1).to_string();
        let view: ReadView = self.call(&["read", "--json", "--full", "-n", &want, id])?;
        let me = self.identity()?;
        let mut newest_first: Vec<Message> = view
            .rows
            .iter()
            .filter_map(|row| self.message_from(row, None, &me))
            .collect();
        newest_first.reverse();
        let more = newest_first.len() > offset + limit;
        let items: Vec<Message> = newest_first.into_iter().skip(offset).take(limit).collect();
        let next_cursor = more.then(|| (offset + limit).to_string());
        Ok(Page { items, next_cursor })
    }

    /// The root of the thread a message is in. Slack takes only a root for a reply, so a reply to a reply needs this.
    fn root_of(&self, channel: &str, ts: &str) -> CapResult<String> {
        let target = format!("{channel}:{ts}");
        let view: ThreadView = self.call(&["thread", "--json", "--full", "-n", "1", &target])?;
        view.rows
            .first()
            .and_then(|row| parse_permalink(&row.url))
            .map(|link| link.ts)
            .ok_or_else(|| CapError::not_found("the message"))
    }

    fn poll(
        self: Arc<Self>,
        channels: Vec<Ref>,
        mut seen: HashSet<Ref>,
        tx: std::sync::mpsc::Sender<Event>,
        stop: StopFlag,
    ) {
        let slice = Duration::from_millis(10);
        loop {
            let until = Instant::now() + self.config.poll_every;
            while Instant::now() < until {
                if stop.is_stopped() {
                    return;
                }
                thread::sleep(slice.min(self.config.poll_every));
            }
            let mut fresh: Vec<Message> = Vec::new();
            for channel in &channels {
                match self.history(channel, None, Some(PAGE_MAX as u32)) {
                    Ok(page) => fresh.extend(
                        page.items
                            .into_iter()
                            .filter(|m| seen.insert(m.reference.clone())),
                    ),
                    Err(CapError::RateLimited { retry_after_ms }) => {
                        thread::sleep(Duration::from_millis(retry_after_ms.min(60_000)));
                    }
                    Err(_) => {}
                }
            }
            fresh.sort_by_key(|m| m.created_at);
            for message in fresh {
                let event = Event {
                    kind: EventKind::Posted,
                    channel: message.channel.clone(),
                    message: message.reference.clone(),
                    by: Some(message.author.clone()),
                    reaction: None,
                    data: Some(message),
                };
                if stop.is_stopped() || tx.send(event).is_err() {
                    return;
                }
            }
        }
    }
}

impl MessagingProvider for SlackMessaging {
    fn provider(&self) -> &str {
        "slack"
    }

    fn account(&self) -> &str {
        self.shared.account()
    }

    fn capabilities(&self) -> MessagingCapabilities {
        let mut operations = MessagingCapabilities::CORE.to_vec();
        operations.extend([Operation::Search, Operation::Edit, Operation::Person]);
        MessagingCapabilities {
            operations,
            features: vec![Feature::Threads, Feature::Edits, Feature::Attachments],
            formatting: Formatting::Basic,
            limits: Limits {
                page_max: Some(PAGE_MAX as u32),
                per_minute: None,
            },
            auth: vec![AuthKind::BrowserSession],
        }
    }

    fn whoami(&self) -> CapResult<Actor> {
        self.shared.me()
    }

    fn workspace(&self) -> CapResult<Workspace> {
        let id = self.shared.identity()?;
        Ok(Workspace {
            reference: workspace_ref(&self.shared.ref_of("")),
            name: id.workspace.clone().unwrap_or_else(|| id.team_id.clone()),
            raw: serde_json::to_value(&id).ok(),
        })
    }

    fn channels(&self, query: &ChannelQuery) -> CapResult<Page<Channel>> {
        let text = query.text.as_deref().map(str::to_lowercase);
        let all: Vec<Channel> = self
            .shared
            .config
            .channels
            .iter()
            .filter(|c| query.kinds.is_empty() || query.kinds.contains(&c.kind))
            .filter(|c| {
                text.as_ref()
                    .is_none_or(|t| c.name.to_lowercase().contains(t))
            })
            .map(|c| Channel {
                reference: self.shared.ref_of(&c.id),
                name: c.name.clone(),
                kind: c.kind,
                topic: None,
                archived: false,
                member_count: None,
                unread: None,
                created_at: None,
                raw: None,
            })
            .collect();
        let start = offset_of(query.cursor.as_deref())?;
        let limit = limit_of(query.limit);
        let items: Vec<Channel> = all.iter().skip(start).take(limit).cloned().collect();
        let next_cursor =
            (start + items.len() < all.len()).then(|| (start + items.len()).to_string());
        Ok(Page { items, next_cursor })
    }

    fn history(
        &self,
        channel: &Ref,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> CapResult<Page<Message>> {
        self.shared.history(channel, cursor, limit)
    }

    /// The root and its latest 100 replies, oldest first. A longer thread shows its newest replies: `slackcli` has no way
    /// to ask for an earlier page.
    fn thread(&self, root: &Ref, cursor: Option<&str>) -> CapResult<Page<Message>> {
        let (channel, ts) = self.shared.message_target(root, "message")?;
        if cursor.is_some() {
            return Err(CapError::invalid("cursor"));
        }
        let target = format!("{channel}:{ts}");
        let view: ThreadView = self.shared.call(&[
            "thread",
            "--json",
            "--full",
            "-n",
            &PAGE_MAX.to_string(),
            &target,
        ])?;
        let me = self.shared.identity()?;
        let mut rows = view.rows.iter();
        let head = rows
            .next()
            .ok_or_else(|| CapError::not_found("the message"))?;
        let mut root_message = self
            .shared
            .message_from(head, None, &me)
            .ok_or_else(|| CapError::not_found("the message"))?;
        let replies: Vec<Message> = rows
            .filter_map(|row| {
                self.shared
                    .message_from(row, Some(root_message.reference.clone()), &me)
            })
            .collect();
        if root_message.reply_count == 0 {
            root_message.reply_count = replies.len() as u32;
        }
        let mut items = vec![root_message];
        items.extend(replies);
        Ok(Page {
            items,
            next_cursor: None,
        })
    }

    fn send(&self, new: &NewMessage, by: &Actor) -> CapResult<Message> {
        let channel = self.shared.channel_id(&new.channel)?.to_string();
        if new.text.trim().is_empty() {
            return Err(CapError::invalid("text"));
        }
        let me = self.shared.me()?;
        let origin = origin_of(by, &me.name);
        let mut text = to_mrkdwn(&new.text);
        if let Some(origin) = &origin {
            text.push_str(&format!("\n\n_sent by {}_", to_mrkdwn(origin)));
        }
        let (posted, parent): (Posted, Option<Ref>) = match &new.in_thread_of {
            None => (
                self.shared
                    .call(&["send", "--yes", "--json", &channel, "--", &text])?,
                None,
            ),
            Some(target) => {
                let (target_channel, ts) = self.shared.message_target(target, "in_thread_of")?;
                if target_channel != channel {
                    return Err(CapError::invalid("in_thread_of"));
                }
                let root = self.shared.root_of(&channel, &ts)?;
                let to = format!("{channel}:{root}");
                let posted = self
                    .shared
                    .call(&["reply", "--yes", "--json", &to, "--", &text])?;
                (posted, Some(message_ref(&new.channel, &root)))
            }
        };
        Ok(Message {
            reference: message_ref(&new.channel, &posted.ts),
            channel: new.channel.clone(),
            parent,
            text: new.text.clone(),
            author: by.clone(),
            created_at: ts_to_ms(&posted.ts).unwrap_or_else(now_ms),
            edited_at: None,
            attachments: Vec::new(),
            reactions: Vec::new(),
            reply_count: 0,
            origin,
            raw: serde_json::to_value(&posted).ok(),
        })
    }

    /// Looks for new top-level messages every `poll_every`. It sees no edit, delete or reaction, and no reply.
    fn subscribe(&self, filter: &Filter) -> CapResult<Subscription<Event>> {
        let channels: Vec<Ref> = if filter.channels.is_empty() {
            self.shared
                .config
                .channels
                .iter()
                .map(|c| self.shared.ref_of(&c.id))
                .collect()
        } else {
            filter.channels.clone()
        };
        let mut seen = HashSet::new();
        for channel in &channels {
            let page = self.shared.history(channel, None, Some(PAGE_MAX as u32))?;
            seen.extend(page.items.into_iter().map(|m| m.reference));
        }
        let (tx, rx) = channel();
        let (subscription, stop) = Subscription::new(rx);
        let shared = self.shared.clone();
        thread::spawn(move || shared.poll(channels, seen, tx, stop));
        Ok(subscription)
    }

    fn search(&self, query: &SearchQuery) -> CapResult<Page<Message>> {
        if query.text.trim().is_empty() {
            return Err(CapError::invalid("text"));
        }
        let mut terms = vec![query.text.trim().to_string()];
        if let Some(channel) = &query.channel {
            let id = self.shared.channel_id(channel)?;
            let spec = self
                .shared
                .config
                .channels
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| CapError::invalid("channel"))?;
            terms.push(format!("in:{}", spec.name));
        }
        if let Some(from) = &query.from {
            terms.push(format!("from:@{from}"));
        }
        let text = terms.join(" ");
        let offset = offset_of(query.cursor.as_deref())?;
        let limit = limit_of(query.limit);
        let want = (offset + limit + 1).to_string();
        let view: SearchView = self
            .shared
            .call(&["search", "--json", "--full", "-n", &want, "--", &text])?;
        let me = self.shared.identity()?;
        let found: Vec<Message> = view
            .rows
            .iter()
            .filter_map(|row| {
                let message_row = MessageRow {
                    author: row.author.clone(),
                    message: row.message.clone(),
                    at: row.at.clone(),
                    url: row.url.clone()?,
                    replies: None,
                    files: row.files.clone(),
                };
                self.shared.message_from(&message_row, None, &me)
            })
            .collect();
        let more = found.len() > offset + limit;
        let items: Vec<Message> = found.into_iter().skip(offset).take(limit).collect();
        let next_cursor = more.then(|| (offset + limit).to_string());
        Ok(Page { items, next_cursor })
    }

    fn edit(&self, message: &Ref, text: &str, by: &Actor) -> CapResult<Message> {
        let (channel, ts) = self.shared.message_target(message, "message")?;
        if text.trim().is_empty() {
            return Err(CapError::invalid("text"));
        }
        let me = self.shared.me()?;
        let origin = origin_of(by, &me.name);
        let mut body = to_mrkdwn(text);
        if let Some(origin) = &origin {
            body.push_str(&format!("\n\n_sent by {}_", to_mrkdwn(origin)));
        }
        let target = format!("{channel}:{ts}");
        let posted: Posted = self
            .shared
            .call(&["edit", "--yes", "--json", &target, "--", &body])?;
        let channel_ref = self.shared.ref_of(&channel);
        Ok(Message {
            reference: message_ref(&channel_ref, &posted.ts),
            channel: channel_ref,
            parent: None,
            text: text.to_string(),
            author: by.clone(),
            created_at: ts_to_ms(&posted.ts).unwrap_or_else(now_ms),
            edited_at: Some(now_ms()),
            attachments: Vec::new(),
            reactions: Vec::new(),
            reply_count: 0,
            origin,
            raw: serde_json::to_value(&posted).ok(),
        })
    }

    /// Reads a person by the id in `user/<id>`: a Slack user id, or the name `slackcli` printed as an author.
    fn person(&self, person: &Ref) -> CapResult<Person> {
        self.shared.own(person, "person")?;
        let id = person
            .id
            .strip_prefix("user/")
            .filter(|id| !id.is_empty())
            .ok_or_else(|| CapError::invalid("person"))?;
        let looks_like_id = id.len() > 1
            && matches!(id.as_bytes()[0], b'U' | b'W')
            && id
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
        let view: UsersView = if looks_like_id {
            self.shared.call(&["users", "--json", "-n", "1000"])?
        } else {
            self.shared
                .call(&["users", "--json", "-n", "20", "--", id])?
        };
        let found: &UserRow = view
            .rows
            .iter()
            .find(|u| u.id == id || u.handle == id || u.real_name.as_deref() == Some(id))
            .ok_or_else(|| CapError::not_found("the person"))?;
        Ok(Person {
            reference: person_ref(person, &found.id),
            name: found
                .real_name
                .clone()
                .unwrap_or_else(|| found.handle.clone()),
            handle: found.handle.clone(),
            display_name: None,
            is_bot: found.bot,
            avatar_url: None,
            raw: serde_json::to_value(found).ok(),
        })
    }
}
