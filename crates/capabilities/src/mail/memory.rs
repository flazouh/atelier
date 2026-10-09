use std::{
    cmp::Reverse,
    collections::BTreeMap,
    sync::{
        Mutex, MutexGuard,
        mpsc::{Sender, channel},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    helpers::{
        attachment_ref, check_send_approval, draft_ref, kind_of, local_id, mailbox_ref,
        message_ref, reply_recipients, reply_subject, thread_ref,
    },
    structs::{
        Account, Approval, Attachment, Contact, Draft, DraftPatch, Flags, Incoming,
        MailCapabilities, MailEvent, Mailbox, Message, NewDraft, SearchQuery, Thread,
        ThreadSummary,
    },
    traits::MailProvider,
    types::{MailEventKind, MailFeature, MailOperation, MailboxKind, RefKind, Role, SearchSyntax},
};
use crate::{
    Actor, AuthKind, CapError, CapResult, Limits, Ref, StopFlag, Subscription, tasks::Page,
};

const PAGE_DEFAULT: usize = 50;
const PAGE_MAX: usize = 100;

/// The mailboxes every memory account has: id, name, role.
const SYSTEM: [(&str, &str, Role); 6] = [
    ("inbox", "Inbox", Role::Inbox),
    ("sent", "Sent", Role::Sent),
    ("drafts", "Drafts", Role::Drafts),
    ("trash", "Trash", Role::Trash),
    ("spam", "Spam", Role::Spam),
    ("archive", "Archive", Role::Archive),
];

/// The one label a memory account starts with, so the label calls have something to use.
const LABEL: (&str, &str) = ("label-receipts", "Receipts");

/// A mail provider that keeps everything in memory. It is the reference for the contract suite, and a stand-in for tests of
/// the screen and the agent tools. It has every operation. It sends nothing: a sent message only lands in its Sent mailbox.
pub struct MemoryMail {
    address: String,
    clock: Box<dyn Fn() -> i64 + Send + Sync>,
    inner: Mutex<Inner>,
}

struct Stored {
    message: Message,
    /// The bytes of the attachments, by attachment reference.
    files: Vec<(Ref, Vec<u8>)>,
    /// Changes with every change of the message, so a thread can show a version.
    rev: u64,
}

#[derive(Default)]
struct Inner {
    messages: Vec<Stored>,
    drafts: Vec<Draft>,
    counter: u64,
    subscribers: Vec<(Sender<MailEvent>, StopFlag)>,
}

fn system_clock() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn one_line(text: &str, limit: usize) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(limit)
        .collect()
}

impl MemoryMail {
    /// An empty account for `address`.
    pub fn new(address: &str) -> Self {
        Self {
            address: address.to_string(),
            clock: Box::new(system_clock),
            inner: Mutex::new(Inner::default()),
        }
    }

    /// Reads time from `clock`, in milliseconds, so a test moves it.
    pub fn with_clock(mut self, clock: impl Fn() -> i64 + Send + Sync + 'static) -> Self {
        self.clock = Box::new(clock);
        self
    }

    fn me(&self) -> Contact {
        Contact::new(&self.address)
    }

    fn mailbox(&self, id: &str) -> Ref {
        mailbox_ref("memory", &self.address, id)
    }

    fn lock(&self) -> CapResult<MutexGuard<'_, Inner>> {
        self.inner.lock().map_err(|_| CapError::Storage {
            message: "the memory store is poisoned".into(),
        })
    }

    /// Checks that `reference` is a mail reference of this account and of the kind asked for.
    fn own(&self, reference: &Ref, kind: RefKind) -> CapResult<()> {
        if reference.provider == "memory"
            && reference.account == self.address
            && kind_of(reference) == Some(kind)
        {
            Ok(())
        } else {
            Err(CapError::invalid("ref"))
        }
    }

    fn known_mailbox(&self, reference: &Ref) -> CapResult<Role> {
        self.own(reference, RefKind::Mailbox)?;
        let id = local_id(reference);
        if let Some((_, _, role)) = SYSTEM.iter().find(|(sys, ..)| *sys == id) {
            Ok(*role)
        } else if id == LABEL.0 {
            Ok(Role::Custom)
        } else {
            Err(CapError::not_found(reference.to_string()))
        }
    }

    fn next(inner: &mut Inner) -> u64 {
        inner.counter += 1;
        inner.counter
    }

    fn tell(inner: &mut Inner, event: MailEvent) {
        inner
            .subscribers
            .retain(|(tx, stop)| !stop.is_stopped() && tx.send(event.clone()).is_ok());
    }

    fn summary(&self, inner: &Inner, thread: &Ref) -> Option<ThreadSummary> {
        let all: Vec<&Stored> = inner
            .messages
            .iter()
            .filter(|s| &s.message.thread == thread)
            .collect();
        let first = all.first()?;
        let last = all.iter().max_by_key(|s| (s.message.date, s.rev))?;
        let mut participants: Vec<Contact> = Vec::new();
        for c in all
            .iter()
            .flat_map(|s| std::iter::once(&s.message.from).chain(&s.message.to))
        {
            if !participants
                .iter()
                .any(|p| p.address.eq_ignore_ascii_case(&c.address))
            {
                participants.push(c.clone());
            }
        }
        let mut mailboxes: Vec<Ref> = all
            .iter()
            .flat_map(|s| s.message.mailboxes.iter().cloned())
            .collect();
        mailboxes.sort();
        mailboxes.dedup();
        Some(ThreadSummary {
            reference: thread.clone(),
            subject: first.message.subject.clone(),
            snippet: last.message.snippet.clone(),
            participants,
            message_count: all.len() as u32,
            unread: all.iter().filter(|s| !s.message.flags.read).count() as u32,
            starred: all.iter().any(|s| s.message.flags.starred),
            has_attachments: all.iter().any(|s| !s.message.attachments.is_empty()),
            mailboxes,
            last_at: last.message.date,
            version: all.iter().map(|s| s.rev).max().unwrap_or(0).to_string(),
        })
    }

    /// Delivers a message into the inbox, as if it came in. It is not part of [`MailProvider`]: a test and a screen mock use it.
    pub fn receive(&self, incoming: &Incoming) -> CapResult<Message> {
        let mut inner = self.lock()?;
        let thread = match &incoming.thread {
            Some(t) => {
                self.own(t, RefKind::Thread)?;
                if !inner.messages.iter().any(|s| &s.message.thread == t) {
                    return Err(CapError::not_found(t.to_string()));
                }
                t.clone()
            }
            None => {
                let n = Self::next(&mut inner);
                thread_ref("memory", &self.address, &n.to_string())
            }
        };
        let n = Self::next(&mut inner);
        let reference = message_ref("memory", &self.address, &n.to_string());
        let mut files = Vec::new();
        let mut attachments = Vec::new();
        for (index, (filename, mime, bytes)) in incoming.attachments.iter().enumerate() {
            let a = attachment_ref("memory", &self.address, &format!("{n}.{index}"));
            attachments.push(Attachment {
                reference: a.clone(),
                filename: filename.clone(),
                mime: mime.clone(),
                size: Some(bytes.len() as u64),
                inline: false,
            });
            files.push((a, bytes.clone()));
        }
        let message = Message {
            reference,
            thread: thread.clone(),
            from: incoming.from.clone(),
            to: incoming.to.clone(),
            cc: incoming.cc.clone(),
            bcc: vec![],
            reply_to: None,
            subject: incoming.subject.clone(),
            snippet: one_line(&incoming.text, 120),
            text: incoming.text.clone(),
            html: incoming.html.clone(),
            attachments,
            date: incoming.date,
            flags: Flags::default(),
            mailboxes: vec![self.mailbox("inbox")],
            headers: BTreeMap::from([("message-id".to_string(), format!("<m{n}@memory>"))]),
            raw: None,
        };
        let rev = Self::next(&mut inner);
        inner.messages.push(Stored {
            message: message.clone(),
            files,
            rev,
        });
        if let Some(summary) = self.summary(&inner, &thread) {
            Self::tell(
                &mut inner,
                MailEvent {
                    kind: MailEventKind::NewMessage,
                    thread: summary,
                },
            );
        }
        Ok(message)
    }

    /// The positions of the messages a thread or message reference means.
    fn targets(&self, inner: &Inner, target: &Ref) -> CapResult<Vec<usize>> {
        let kind = kind_of(target);
        if !matches!(kind, Some(RefKind::Thread | RefKind::Message)) {
            return Err(CapError::invalid("ref"));
        }
        self.own(target, kind.unwrap_or(RefKind::Thread))?;
        let found: Vec<usize> = inner
            .messages
            .iter()
            .enumerate()
            .filter(|(_, s)| &s.message.reference == target || &s.message.thread == target)
            .map(|(at, _)| at)
            .collect();
        if found.is_empty() {
            Err(CapError::not_found(target.to_string()))
        } else {
            Ok(found)
        }
    }

    /// Applies `change` to each target message. When any message changed, the thread is told as changed.
    fn change(
        &self,
        target: &Ref,
        change: impl Fn(&mut Message, &dyn Fn(&str) -> Ref) -> bool,
    ) -> CapResult<()> {
        let mut inner = self.lock()?;
        let at = self.targets(&inner, target)?;
        let mailbox = |id: &str| self.mailbox(id);
        let mut touched = Vec::new();
        for i in at {
            if change(&mut inner.messages[i].message, &mailbox) {
                touched.push(i);
            }
        }
        let Some(first) = touched.first().copied() else {
            return Ok(());
        };
        let rev = Self::next(&mut inner);
        for i in touched {
            inner.messages[i].rev = rev;
        }
        let thread = inner.messages[first].message.thread.clone();
        if let Some(summary) = self.summary(&inner, &thread) {
            Self::tell(
                &mut inner,
                MailEvent {
                    kind: MailEventKind::Changed,
                    thread: summary,
                },
            );
        }
        Ok(())
    }

    fn check_contacts(contacts: &[Contact], field: &str) -> CapResult<()> {
        if contacts.iter().all(|c| c.address.contains('@')) {
            Ok(())
        } else {
            Err(CapError::invalid(field))
        }
    }
}

fn clear_system(message: &mut Message, system: &[Ref]) {
    message.mailboxes.retain(|m| !system.contains(m));
}

impl MailProvider for MemoryMail {
    fn provider(&self) -> &str {
        "memory"
    }

    fn account(&self) -> &str {
        &self.address
    }

    fn capabilities(&self) -> MailCapabilities {
        MailCapabilities {
            operations: MailOperation::ALL.to_vec(),
            features: vec![
                MailFeature::Threads,
                MailFeature::Labels,
                MailFeature::Drafts,
                MailFeature::Attachments,
                MailFeature::Push,
                MailFeature::StableMessageIds,
            ],
            search_syntax: SearchSyntax::Plain,
            limits: Limits {
                page_max: Some(PAGE_MAX as u32),
                per_minute: None,
            },
            auth: vec![AuthKind::None],
        }
    }

    fn whoami(&self) -> CapResult<Account> {
        Ok(Account {
            reference: Ref {
                capability: "mail".into(),
                provider: "memory".into(),
                account: self.address.clone(),
                id: "account".into(),
            },
            address: self.address.clone(),
            name: None,
        })
    }

    fn mailboxes(&self) -> CapResult<Vec<Mailbox>> {
        let inner = self.lock()?;
        let count = |reference: &Ref| {
            let in_box = inner
                .messages
                .iter()
                .filter(|s| s.message.mailboxes.contains(reference));
            let total = in_box.clone().count() as u32;
            (
                in_box.filter(|s| !s.message.flags.read).count() as u32,
                total,
            )
        };
        let boxes = SYSTEM
            .iter()
            .map(|(id, name, role)| (*id, *name, *role, MailboxKind::System))
            .chain([(LABEL.0, LABEL.1, Role::Custom, MailboxKind::User)]);
        Ok(boxes
            .map(|(id, name, role, kind)| {
                let reference = self.mailbox(id);
                let (unread, total) = count(&reference);
                Mailbox {
                    reference,
                    name: name.to_string(),
                    role,
                    kind,
                    unread,
                    total: Some(total),
                    color: None,
                    raw: None,
                }
            })
            .collect())
    }

    fn search(&self, query: &SearchQuery) -> CapResult<Page<ThreadSummary>> {
        let start = match &query.cursor {
            Some(cursor) => cursor
                .parse::<usize>()
                .map_err(|_| CapError::invalid("cursor"))?,
            None => 0,
        };
        if let Some(mailbox) = &query.mailbox {
            self.known_mailbox(mailbox)?;
        }
        let inner = self.lock()?;
        let hidden = [self.mailbox("trash"), self.mailbox("spam")];
        let words: Vec<String> = query
            .text
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let eligible = |s: &Stored| match &query.mailbox {
            Some(m) => s.message.mailboxes.contains(m),
            None => !s.message.mailboxes.iter().any(|m| hidden.contains(m)),
        };
        let matches = |s: &Stored| {
            let m = &s.message;
            let haystack = format!(
                "{} {} {} {} {}",
                m.subject,
                m.text,
                m.from.address,
                m.from.name.as_deref().unwrap_or_default(),
                m.to.iter()
                    .map(|c| c.address.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
            .to_lowercase();
            (!query.unread || !m.flags.read) && words.iter().all(|w| haystack.contains(w))
        };
        let mut threads: Vec<Ref> = Vec::new();
        for s in inner.messages.iter().filter(|s| eligible(s) && matches(s)) {
            if !threads.contains(&s.message.thread) {
                threads.push(s.message.thread.clone());
            }
        }
        let mut found: Vec<ThreadSummary> = threads
            .iter()
            .filter_map(|t| self.summary(&inner, t))
            .collect();
        found.sort_by_key(|t| {
            (
                Reverse(t.last_at),
                Reverse(local_id(&t.reference).parse::<u64>().unwrap_or(0)),
            )
        });
        let size = query
            .limit
            .map_or(PAGE_DEFAULT, |n| (n as usize).clamp(1, PAGE_MAX));
        let end = (start + size).min(found.len());
        let items = found.get(start..end).unwrap_or_default().to_vec();
        Ok(Page {
            items,
            next_cursor: (end < found.len()).then(|| end.to_string()),
        })
    }

    fn thread(&self, thread: &Ref) -> CapResult<Thread> {
        self.own(thread, RefKind::Thread)?;
        let inner = self.lock()?;
        let summary = self
            .summary(&inner, thread)
            .ok_or_else(|| CapError::not_found(thread.to_string()))?;
        let mut messages: Vec<&Stored> = inner
            .messages
            .iter()
            .filter(|s| &s.message.thread == thread)
            .collect();
        messages.sort_by_key(|s| (s.message.date, s.rev));
        Ok(Thread {
            summary,
            messages: messages.into_iter().map(|s| s.message.clone()).collect(),
        })
    }

    fn get(&self, message: &Ref) -> CapResult<Message> {
        self.own(message, RefKind::Message)?;
        self.lock()?
            .messages
            .iter()
            .find(|s| &s.message.reference == message)
            .map(|s| s.message.clone())
            .ok_or_else(|| CapError::not_found(message.to_string()))
    }

    fn draft(&self, draft: &Ref) -> CapResult<Draft> {
        self.own(draft, RefKind::Draft)?;
        self.lock()?
            .drafts
            .iter()
            .find(|d| &d.reference == draft)
            .cloned()
            .ok_or_else(|| CapError::not_found(draft.to_string()))
    }

    fn mark_read(&self, target: &Ref, read: bool, _by: &Actor) -> CapResult<()> {
        self.change(target, |m, _| {
            let changed = m.flags.read != read;
            m.flags.read = read;
            changed
        })
    }

    fn star(&self, target: &Ref, starred: bool, _by: &Actor) -> CapResult<()> {
        self.change(target, |m, _| {
            let changed = m.flags.starred != starred;
            m.flags.starred = starred;
            changed
        })
    }

    fn archive(&self, target: &Ref, _by: &Actor) -> CapResult<()> {
        self.change(target, |m, mailbox| {
            let inbox = mailbox("inbox");
            if !m.mailboxes.contains(&inbox) {
                return false;
            }
            m.mailboxes.retain(|b| b != &inbox);
            if m.mailboxes.is_empty() {
                m.mailboxes.push(mailbox("archive"));
            }
            true
        })
    }

    fn label(&self, target: &Ref, add: &[Ref], remove: &[Ref], _by: &Actor) -> CapResult<()> {
        for reference in add.iter().chain(remove) {
            if self.known_mailbox(reference)? != Role::Custom {
                return Err(CapError::invalid("label"));
            }
        }
        self.change(target, |m, _| {
            let before = m.mailboxes.clone();
            m.mailboxes.retain(|b| !remove.contains(b));
            for label in add {
                if !m.mailboxes.contains(label) {
                    m.mailboxes.push(label.clone());
                }
            }
            before != m.mailboxes
        })
    }

    fn move_to(&self, target: &Ref, mailbox: &Ref, _by: &Actor) -> CapResult<()> {
        self.known_mailbox(mailbox)?;
        let system: Vec<Ref> = SYSTEM.iter().map(|(id, ..)| self.mailbox(id)).collect();
        self.change(target, |m, _| {
            let before = m.mailboxes.clone();
            clear_system(m, &system);
            if !m.mailboxes.contains(mailbox) {
                m.mailboxes.push(mailbox.clone());
            }
            before != m.mailboxes
        })
    }

    fn trash(&self, target: &Ref, by: &Actor) -> CapResult<()> {
        self.move_to(target, &self.mailbox("trash"), by)
    }

    fn create_draft(&self, new: &NewDraft, by: &Actor) -> CapResult<Draft> {
        Self::check_contacts(&new.to, "to")?;
        Self::check_contacts(&new.cc, "cc")?;
        Self::check_contacts(&new.bcc, "bcc")?;
        let mut inner = self.lock()?;
        let thread = match &new.in_reply_to {
            Some(original) => {
                self.own(original, RefKind::Message)?;
                let found = inner
                    .messages
                    .iter()
                    .find(|s| &s.message.reference == original)
                    .ok_or_else(|| CapError::not_found(original.to_string()))?;
                Some(found.message.thread.clone())
            }
            None => None,
        };
        let n = Self::next(&mut inner);
        let now = (self.clock)();
        let draft = Draft {
            reference: draft_ref("memory", &self.address, &n.to_string()),
            thread,
            in_reply_to: new.in_reply_to.clone(),
            to: new.to.clone(),
            cc: new.cc.clone(),
            bcc: new.bcc.clone(),
            subject: new.subject.clone(),
            text: new.text.clone(),
            created_by: by.clone(),
            created_at: now,
            updated_at: now,
            version: n.to_string(),
        };
        inner.drafts.push(draft.clone());
        Ok(draft)
    }

    fn update_draft(
        &self,
        draft: &Ref,
        patch: &DraftPatch,
        version: &str,
        _by: &Actor,
    ) -> CapResult<Draft> {
        self.own(draft, RefKind::Draft)?;
        for (field, list) in [("to", &patch.to), ("cc", &patch.cc), ("bcc", &patch.bcc)] {
            if let Some(list) = list {
                Self::check_contacts(list, field)?;
            }
        }
        let mut inner = self.lock()?;
        let at = inner
            .drafts
            .iter()
            .position(|d| &d.reference == draft)
            .ok_or_else(|| CapError::not_found(draft.to_string()))?;
        if inner.drafts[at].version != version {
            return Err(CapError::Conflict {
                current: serde_json::to_value(&inner.drafts[at]).unwrap_or_default(),
            });
        }
        let before = inner.drafts[at].clone();
        let mut next = before.clone();
        if let Some(to) = &patch.to {
            next.to = to.clone();
        }
        if let Some(cc) = &patch.cc {
            next.cc = cc.clone();
        }
        if let Some(bcc) = &patch.bcc {
            next.bcc = bcc.clone();
        }
        if let Some(subject) = &patch.subject {
            next.subject = subject.clone();
        }
        if let Some(text) = &patch.text {
            next.text = text.clone();
        }
        if next == before {
            return Ok(before);
        }
        next.updated_at = (self.clock)();
        next.version = Self::next(&mut inner).to_string();
        inner.drafts[at] = next.clone();
        Ok(next)
    }

    fn reply(&self, message: &Ref, all: bool, text: &str, by: &Actor) -> CapResult<Draft> {
        if text.trim().is_empty() {
            return Err(CapError::invalid("text"));
        }
        let original = self.get(message)?;
        let (to, cc) = reply_recipients(&original, &self.address, all);
        self.create_draft(
            &NewDraft {
                to,
                cc,
                bcc: vec![],
                subject: reply_subject(&original.subject),
                text: text.to_string(),
                in_reply_to: Some(message.clone()),
            },
            by,
        )
    }

    fn send(
        &self,
        draft: &Ref,
        version: &str,
        by: &Actor,
        approval: Option<&Approval>,
    ) -> CapResult<Message> {
        self.own(draft, RefKind::Draft)?;
        let mut inner = self.lock()?;
        let at = inner
            .drafts
            .iter()
            .position(|d| &d.reference == draft)
            .ok_or_else(|| CapError::not_found(draft.to_string()))?;
        let found = inner.drafts[at].clone();
        if found.version != version {
            return Err(CapError::Conflict {
                current: serde_json::to_value(&found).unwrap_or_default(),
            });
        }
        check_send_approval(by, &found.version, approval)?;
        if found.to.is_empty() && found.cc.is_empty() && found.bcc.is_empty() {
            return Err(CapError::invalid("to"));
        }
        let thread = match &found.thread {
            Some(t) => t.clone(),
            None => {
                let n = Self::next(&mut inner);
                thread_ref("memory", &self.address, &n.to_string())
            }
        };
        let n = Self::next(&mut inner);
        let mut headers = BTreeMap::from([("message-id".to_string(), format!("<m{n}@memory>"))]);
        if let Some(original) = &found.in_reply_to
            && let Some(id) = inner
                .messages
                .iter()
                .find(|s| &s.message.reference == original)
                .and_then(|s| s.message.headers.get("message-id"))
        {
            headers.insert("in-reply-to".into(), id.clone());
        }
        let message = Message {
            reference: message_ref("memory", &self.address, &n.to_string()),
            thread: thread.clone(),
            from: self.me(),
            to: found.to.clone(),
            cc: found.cc.clone(),
            bcc: found.bcc.clone(),
            reply_to: None,
            subject: found.subject.clone(),
            snippet: one_line(&found.text, 120),
            text: found.text.clone(),
            html: None,
            attachments: vec![],
            date: (self.clock)(),
            flags: Flags {
                read: true,
                starred: false,
            },
            mailboxes: vec![self.mailbox("sent")],
            headers,
            raw: None,
        };
        let rev = Self::next(&mut inner);
        inner.drafts.remove(at);
        inner.messages.push(Stored {
            message: message.clone(),
            files: vec![],
            rev,
        });
        if let Some(summary) = self.summary(&inner, &thread) {
            Self::tell(
                &mut inner,
                MailEvent {
                    kind: MailEventKind::Changed,
                    thread: summary,
                },
            );
        }
        Ok(message)
    }

    fn download_attachment(&self, attachment: &Ref) -> CapResult<Vec<u8>> {
        self.own(attachment, RefKind::Attachment)?;
        self.lock()?
            .messages
            .iter()
            .flat_map(|s| s.files.iter())
            .find(|(r, _)| r == attachment)
            .map(|(_, bytes)| bytes.clone())
            .ok_or_else(|| CapError::not_found(attachment.to_string()))
    }

    fn subscribe(&self) -> CapResult<Subscription<MailEvent>> {
        let (tx, rx) = channel();
        let (subscription, stop) = Subscription::new(rx);
        self.lock()?.subscribers.push((tx, stop));
        Ok(subscription)
    }
}
