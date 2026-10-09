use std::{
    sync::{
        Mutex, MutexGuard,
        mpsc::{Sender, channel},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    helpers::{message_ref, origin_of, person_ref, require_message, workspace_ref},
    structs::{
        Channel, ChannelQuery, Event, Filter, Message, MessagingCapabilities, NewMessage, Page,
        Person, Reaction, SearchQuery, Workspace,
    },
    traits::MessagingProvider,
    types::{ChannelKind, EventKind, Feature, Formatting, Operation},
};
use crate::{Actor, ActorKind, AuthKind, CapError, CapResult, Limits, Ref, StopFlag, Subscription};

const PAGE_DEFAULT: usize = 50;
const PAGE_MAX: usize = 100;

/// A messaging provider that keeps everything in memory. It is the reference for the contract suite, and a stand-in for
/// the screen and the agent tools. It lists every optional call except `export` and `import`.
///
/// It sends as one person, `me`. Deleting a root message deletes its replies too.
pub struct MemoryMessaging {
    account: String,
    me: Actor,
    clock: Box<dyn Fn() -> i64 + Send + Sync>,
    inner: Mutex<Inner>,
}

struct Stored {
    /// Counts up over the whole provider, so it orders messages and makes a stable cursor.
    seq: u64,
    message: Message,
}

#[derive(Default)]
struct Inner {
    channels: Vec<Channel>,
    messages: Vec<Stored>,
    people: Vec<Actor>,
    counter: u64,
    subscribers: Vec<(Sender<Event>, StopFlag, Vec<Ref>)>,
}

fn system_clock() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// The id of the person whose credentials an actor uses: an agent uses its person's.
fn acting_id(by: &Actor) -> &str {
    match (by.kind, &by.on_behalf_of) {
        (ActorKind::Agent, Some(owner)) => owner,
        _ => &by.id,
    }
}

fn limit_of(limit: Option<u32>) -> usize {
    (limit.map_or(PAGE_DEFAULT, |n| n as usize)).clamp(1, PAGE_MAX)
}

impl MemoryMessaging {
    pub fn new(account: &str) -> Self {
        let me = Actor::person("me", "Me");
        Self {
            account: account.to_string(),
            inner: Mutex::new(Inner {
                people: vec![me.clone()],
                ..Inner::default()
            }),
            me,
            clock: Box::new(system_clock),
        }
    }

    /// Reads time from `clock`, in milliseconds, so a test moves it.
    pub fn with_clock(mut self, clock: impl Fn() -> i64 + Send + Sync + 'static) -> Self {
        self.clock = Box::new(clock);
        self
    }

    /// Makes a channel, for a test or a screen that needs something to show.
    pub fn add_channel(&self, name: &str, kind: ChannelKind) -> Ref {
        let mut inner = self.inner.lock().expect("the memory store is poisoned");
        let reference = self.make_ref(&format!("C{}", inner.channels.len() + 1));
        inner.channels.push(Channel {
            reference: reference.clone(),
            name: name.to_string(),
            kind,
            topic: None,
            archived: false,
            member_count: None,
            unread: None,
            created_at: Some((self.clock)()),
            raw: None,
        });
        reference
    }

    fn make_ref(&self, id: &str) -> Ref {
        Ref {
            capability: "messaging".into(),
            provider: "memory".into(),
            account: self.account.clone(),
            id: id.to_string(),
        }
    }

    fn lock(&self) -> CapResult<MutexGuard<'_, Inner>> {
        self.inner.lock().map_err(|_| CapError::Storage {
            message: "the memory store is poisoned".into(),
        })
    }

    fn own(&self, channel: &Ref, field: &str) -> CapResult<()> {
        if channel.capability != "messaging"
            || channel.provider != "memory"
            || channel.account != self.account
        {
            return Err(CapError::invalid(field));
        }
        Ok(())
    }

    fn tell(inner: &mut Inner, event: Event) {
        inner.subscribers.retain(|(tx, stop, channels)| {
            if stop.is_stopped() {
                return false;
            }
            if !channels.is_empty() && !channels.contains(&event.channel) {
                return true;
            }
            tx.send(event.clone()).is_ok()
        });
    }

    fn find<'a>(inner: &'a Inner, message: &Ref) -> CapResult<(usize, &'a Stored)> {
        inner
            .messages
            .iter()
            .enumerate()
            .find(|(_, s)| s.message.reference == *message)
            .ok_or_else(|| CapError::not_found("the message"))
    }

    fn channel_exists(inner: &Inner, channel: &Ref) -> CapResult<()> {
        if inner.channels.iter().any(|c| c.reference == *channel) {
            Ok(())
        } else {
            Err(CapError::not_found("the channel"))
        }
    }

    /// The message with its reply count, as a reader sees it.
    fn view(inner: &Inner, s: &Stored) -> Message {
        let mut m = s.message.clone();
        m.reply_count = inner
            .messages
            .iter()
            .filter(|r| r.message.parent.as_ref() == Some(&m.reference))
            .count() as u32;
        m
    }

    fn owner_name(inner: &Inner, by: &Actor) -> String {
        let id = acting_id(by);
        inner
            .people
            .iter()
            .find(|p| p.id == id)
            .map_or_else(|| id.to_string(), |p| p.name.clone())
    }
}

impl MessagingProvider for MemoryMessaging {
    fn provider(&self) -> &str {
        "memory"
    }

    fn account(&self) -> &str {
        &self.account
    }

    fn capabilities(&self) -> MessagingCapabilities {
        let mut operations = MessagingCapabilities::CORE.to_vec();
        operations.extend([
            Operation::Search,
            Operation::Edit,
            Operation::Delete,
            Operation::React,
            Operation::MarkRead,
            Operation::Person,
        ]);
        MessagingCapabilities {
            operations,
            features: vec![Feature::Threads, Feature::Reactions, Feature::Edits],
            formatting: Formatting::Rich,
            limits: Limits {
                page_max: Some(PAGE_MAX as u32),
                per_minute: None,
            },
            auth: vec![AuthKind::None],
        }
    }

    fn whoami(&self) -> CapResult<Actor> {
        Ok(self.me.clone())
    }

    fn workspace(&self) -> CapResult<Workspace> {
        Ok(Workspace {
            reference: workspace_ref(&self.make_ref("")),
            name: self.account.clone(),
            raw: None,
        })
    }

    fn channels(&self, query: &ChannelQuery) -> CapResult<Page<Channel>> {
        let inner = self.lock()?;
        let text = query.text.as_deref().map(str::to_lowercase);
        let all: Vec<&Channel> = inner
            .channels
            .iter()
            .filter(|c| query.include_archived || !c.archived)
            .filter(|c| query.kinds.is_empty() || query.kinds.contains(&c.kind))
            .filter(|c| {
                text.as_ref()
                    .is_none_or(|t| c.name.to_lowercase().contains(t))
            })
            .collect();
        let start = match &query.cursor {
            None => 0,
            Some(c) => c
                .parse::<usize>()
                .map_err(|_| CapError::invalid("cursor"))?,
        };
        let limit = limit_of(query.limit);
        let items: Vec<Channel> = all
            .iter()
            .skip(start)
            .take(limit)
            .map(|c| (*c).clone())
            .collect();
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
        self.own(channel, "channel")?;
        let inner = self.lock()?;
        Self::channel_exists(&inner, channel)?;
        let before = match cursor {
            None => u64::MAX,
            Some(c) => c.parse::<u64>().map_err(|_| CapError::invalid("cursor"))?,
        };
        let older: Vec<&Stored> = inner
            .messages
            .iter()
            .rev()
            .filter(|s| {
                s.message.channel == *channel && s.message.parent.is_none() && s.seq < before
            })
            .collect();
        let limit = limit_of(limit);
        let items: Vec<Message> = older
            .iter()
            .take(limit)
            .map(|s| Self::view(&inner, s))
            .collect();
        let next_cursor = (older.len() > limit).then(|| older[limit - 1].seq.to_string());
        Ok(Page { items, next_cursor })
    }

    fn thread(&self, root: &Ref, cursor: Option<&str>) -> CapResult<Page<Message>> {
        self.own(root, "message")?;
        require_message(root, "message")?;
        let inner = self.lock()?;
        let (_, found) = Self::find(&inner, root)?;
        let root = found.message.parent.clone().unwrap_or_else(|| root.clone());
        let (_, head) = Self::find(&inner, &root)?;
        let mut all = vec![Self::view(&inner, head)];
        all.extend(
            inner
                .messages
                .iter()
                .filter(|s| s.message.parent.as_ref() == Some(&root))
                .map(|s| Self::view(&inner, s)),
        );
        let start = match cursor {
            None => 0,
            Some(c) => c
                .parse::<usize>()
                .map_err(|_| CapError::invalid("cursor"))?,
        };
        let items: Vec<Message> = all.iter().skip(start).take(PAGE_MAX).cloned().collect();
        let next_cursor =
            (start + items.len() < all.len()).then(|| (start + items.len()).to_string());
        Ok(Page { items, next_cursor })
    }

    fn send(&self, new: &NewMessage, by: &Actor) -> CapResult<Message> {
        self.own(&new.channel, "channel")?;
        if new.text.trim().is_empty() {
            return Err(CapError::invalid("text"));
        }
        let mut inner = self.lock()?;
        Self::channel_exists(&inner, &new.channel)?;
        let parent = match &new.in_thread_of {
            None => None,
            Some(target) => {
                self.own(target, "in_thread_of")?;
                let (_, found) = Self::find(&inner, target)?;
                if found.message.channel != new.channel {
                    return Err(CapError::invalid("in_thread_of"));
                }
                Some(
                    found
                        .message
                        .parent
                        .clone()
                        .unwrap_or_else(|| found.message.reference.clone()),
                )
            }
        };
        if !inner.people.iter().any(|p| p.id == by.id) && by.kind == ActorKind::Person {
            inner.people.push(by.clone());
        }
        inner.counter += 1;
        let seq = inner.counter;
        let now = (self.clock)();
        let ts = format!("{}.{seq:06}", now / 1000);
        let message = Message {
            reference: message_ref(&new.channel, &ts),
            channel: new.channel.clone(),
            parent,
            text: new.text.clone(),
            author: by.clone(),
            created_at: now,
            edited_at: None,
            attachments: Vec::new(),
            reactions: Vec::new(),
            reply_count: 0,
            origin: origin_of(by, &Self::owner_name(&inner, by)),
            raw: None,
        };
        inner.messages.push(Stored {
            seq,
            message: message.clone(),
        });
        Self::tell(
            &mut inner,
            Event {
                kind: EventKind::Posted,
                channel: message.channel.clone(),
                message: message.reference.clone(),
                data: Some(message.clone()),
                reaction: None,
                by: Some(by.clone()),
            },
        );
        Ok(message)
    }

    fn subscribe(&self, filter: &Filter) -> CapResult<Subscription<Event>> {
        let (tx, rx) = channel();
        let (subscription, stop) = Subscription::new(rx);
        self.lock()?
            .subscribers
            .push((tx, stop, filter.channels.clone()));
        Ok(subscription)
    }

    fn search(&self, query: &SearchQuery) -> CapResult<Page<Message>> {
        if query.text.trim().is_empty() {
            return Err(CapError::invalid("text"));
        }
        let inner = self.lock()?;
        let needle = query.text.to_lowercase();
        let before = match &query.cursor {
            None => u64::MAX,
            Some(c) => c.parse::<u64>().map_err(|_| CapError::invalid("cursor"))?,
        };
        let hits: Vec<&Stored> = inner
            .messages
            .iter()
            .rev()
            .filter(|s| s.seq < before)
            .filter(|s| {
                query
                    .channel
                    .as_ref()
                    .is_none_or(|c| s.message.channel == *c)
            })
            .filter(|s| {
                query
                    .from
                    .as_deref()
                    .is_none_or(|f| acting_id(&s.message.author) == f)
            })
            .filter(|s| s.message.text.to_lowercase().contains(&needle))
            .collect();
        let limit = limit_of(query.limit);
        let items: Vec<Message> = hits
            .iter()
            .take(limit)
            .map(|s| Self::view(&inner, s))
            .collect();
        let next_cursor = (hits.len() > limit).then(|| hits[limit - 1].seq.to_string());
        Ok(Page { items, next_cursor })
    }

    fn edit(&self, message: &Ref, text: &str, by: &Actor) -> CapResult<Message> {
        self.own(message, "message")?;
        require_message(message, "message")?;
        if text.trim().is_empty() {
            return Err(CapError::invalid("text"));
        }
        let mut inner = self.lock()?;
        let (at, _) = Self::find(&inner, message)?;
        let now = (self.clock)();
        inner.messages[at].message.text = text.to_string();
        inner.messages[at].message.edited_at = Some(now);
        let view = Self::view(&inner, &inner.messages[at]);
        Self::tell(
            &mut inner,
            Event {
                kind: EventKind::Edited,
                channel: view.channel.clone(),
                message: view.reference.clone(),
                data: Some(view.clone()),
                reaction: None,
                by: Some(by.clone()),
            },
        );
        Ok(view)
    }

    fn delete(&self, message: &Ref, by: &Actor) -> CapResult<()> {
        self.own(message, "message")?;
        require_message(message, "message")?;
        let mut inner = self.lock()?;
        let (_, found) = Self::find(&inner, message)?;
        let channel = found.message.channel.clone();
        let gone: Vec<Ref> = inner
            .messages
            .iter()
            .filter(|s| {
                s.message.reference == *message || s.message.parent.as_ref() == Some(message)
            })
            .map(|s| s.message.reference.clone())
            .collect();
        inner
            .messages
            .retain(|s| !gone.contains(&s.message.reference));
        for reference in gone {
            Self::tell(
                &mut inner,
                Event {
                    kind: EventKind::Deleted,
                    channel: channel.clone(),
                    message: reference,
                    data: None,
                    reaction: None,
                    by: Some(by.clone()),
                },
            );
        }
        Ok(())
    }

    fn react(&self, message: &Ref, name: &str, on: bool, by: &Actor) -> CapResult<Message> {
        self.own(message, "message")?;
        require_message(message, "message")?;
        let name = name.trim().trim_matches(':');
        if name.is_empty() {
            return Err(CapError::invalid("name"));
        }
        let mut inner = self.lock()?;
        let (at, _) = Self::find(&inner, message)?;
        let who = acting_id(by).to_string();
        let me = self.me.id.clone();
        let reactions = &mut inner.messages[at].message.reactions;
        let changed = match reactions.iter().position(|r| r.name == name) {
            Some(i) => {
                let has = reactions[i].by.contains(&who);
                if on && !has {
                    reactions[i].by.push(who.clone());
                } else if !on && has {
                    reactions[i].by.retain(|id| *id != who);
                }
                on != has
            }
            None if on => {
                reactions.push(Reaction {
                    name: name.to_string(),
                    count: 0,
                    by: vec![who.clone()],
                    me: false,
                });
                true
            }
            None => false,
        };
        reactions.retain(|r| !r.by.is_empty());
        for r in reactions.iter_mut() {
            r.count = r.by.len() as u32;
            r.me = r.by.contains(&me);
        }
        let view = Self::view(&inner, &inner.messages[at]);
        if changed {
            Self::tell(
                &mut inner,
                Event {
                    kind: if on {
                        EventKind::ReactionAdded
                    } else {
                        EventKind::ReactionRemoved
                    },
                    channel: view.channel.clone(),
                    message: view.reference.clone(),
                    data: Some(view.clone()),
                    reaction: Some(name.to_string()),
                    by: Some(by.clone()),
                },
            );
        }
        Ok(view)
    }

    fn mark_read(&self, channel: &Ref, up_to: Option<&Ref>) -> CapResult<()> {
        self.own(channel, "channel")?;
        let mut inner = self.lock()?;
        Self::channel_exists(&inner, channel)?;
        if let Some(message) = up_to {
            let (_, found) = Self::find(&inner, message)?;
            if found.message.channel != *channel {
                return Err(CapError::invalid("up_to"));
            }
        }
        if let Some(c) = inner.channels.iter_mut().find(|c| c.reference == *channel) {
            c.unread = Some(0);
        }
        Ok(())
    }

    fn person(&self, person: &Ref) -> CapResult<Person> {
        self.own(person, "person")?;
        let inner = self.lock()?;
        let id = person
            .id
            .strip_prefix("user/")
            .ok_or_else(|| CapError::invalid("person"))?;
        let known = inner
            .people
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| CapError::not_found("the person"))?;
        Ok(Person {
            reference: person_ref(person, id),
            name: known.name.clone(),
            handle: known.id.clone(),
            display_name: None,
            is_bot: false,
            avatar_url: None,
            raw: None,
        })
    }
}
