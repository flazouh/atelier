//! Providers that fail on purpose or lack a call, to see how the tools answer. Each wraps a memory provider.
use std::sync::Mutex;

use atelier_capabilities::{
    Actor, CapError, CapResult, Ref, Subscription,
    mail::{
        Account, Draft, MailCapabilities, MailEvent, MailOperation, MailProvider, Mailbox,
        MemoryMail, NewDraft, SearchQuery as MailQuery, Thread, ThreadSummary,
    },
    messaging::{
        Channel, ChannelQuery, Event, Filter, MemoryMessaging, Message, MessagingCapabilities,
        MessagingProvider, NewMessage, Operation, Page, SearchQuery, Workspace,
    },
    tasks::Page as MailPage,
};

/// A chat account that lists only `operations`, and fails every read and send with `fail` while it is set.
pub struct FakeChat {
    pub inner: MemoryMessaging,
    operations: Vec<Operation>,
    fail: Mutex<Option<CapError>>,
}

impl FakeChat {
    pub fn new(account: &str, operations: &[Operation]) -> Self {
        Self {
            inner: MemoryMessaging::new(account),
            operations: operations.to_vec(),
            fail: Mutex::new(None),
        }
    }

    pub fn failing(&self, error: CapError) {
        *self.fail.lock().unwrap() = Some(error);
    }

    fn check(&self) -> CapResult<()> {
        match self.fail.lock().unwrap().clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl MessagingProvider for FakeChat {
    fn provider(&self) -> &str {
        self.inner.provider()
    }
    fn account(&self) -> &str {
        self.inner.account()
    }
    fn capabilities(&self) -> MessagingCapabilities {
        MessagingCapabilities {
            operations: self.operations.clone(),
            ..self.inner.capabilities()
        }
    }
    fn whoami(&self) -> CapResult<Actor> {
        self.inner.whoami()
    }
    fn workspace(&self) -> CapResult<Workspace> {
        self.inner.workspace()
    }
    fn channels(&self, query: &ChannelQuery) -> CapResult<Page<Channel>> {
        self.check()?;
        self.inner.channels(query)
    }
    fn history(
        &self,
        channel: &Ref,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> CapResult<Page<Message>> {
        self.check()?;
        self.inner.history(channel, cursor, limit)
    }
    fn thread(&self, root: &Ref, cursor: Option<&str>) -> CapResult<Page<Message>> {
        self.check()?;
        self.inner.thread(root, cursor)
    }
    fn send(&self, new: &NewMessage, by: &Actor) -> CapResult<Message> {
        self.check()?;
        self.inner.send(new, by)
    }
    fn subscribe(&self, filter: &Filter) -> CapResult<Subscription<Event>> {
        self.inner.subscribe(filter)
    }
    fn search(&self, query: &SearchQuery) -> CapResult<Page<Message>> {
        self.check()?;
        self.inner.search(query)
    }
}

/// A mail account that lists only `operations`, and fails every read and draft with `fail` while it is set.
pub struct FakeMail {
    pub inner: MemoryMail,
    operations: Vec<MailOperation>,
    fail: Mutex<Option<CapError>>,
}

impl FakeMail {
    pub fn new(address: &str, operations: &[MailOperation]) -> Self {
        Self {
            inner: MemoryMail::new(address),
            operations: operations.to_vec(),
            fail: Mutex::new(None),
        }
    }

    pub fn failing(&self, error: CapError) {
        *self.fail.lock().unwrap() = Some(error);
    }

    fn check(&self) -> CapResult<()> {
        match self.fail.lock().unwrap().clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl MailProvider for FakeMail {
    fn provider(&self) -> &str {
        self.inner.provider()
    }
    fn account(&self) -> &str {
        self.inner.account()
    }
    fn capabilities(&self) -> MailCapabilities {
        MailCapabilities {
            operations: self.operations.clone(),
            ..self.inner.capabilities()
        }
    }
    fn whoami(&self) -> CapResult<Account> {
        self.inner.whoami()
    }
    fn mailboxes(&self) -> CapResult<Vec<Mailbox>> {
        self.check()?;
        self.inner.mailboxes()
    }
    fn search(&self, query: &MailQuery) -> CapResult<MailPage<ThreadSummary>> {
        self.check()?;
        self.inner.search(query)
    }
    fn thread(&self, thread: &Ref) -> CapResult<Thread> {
        self.check()?;
        self.inner.thread(thread)
    }
    fn get(&self, message: &Ref) -> CapResult<atelier_capabilities::mail::Message> {
        self.check()?;
        self.inner.get(message)
    }
    fn create_draft(&self, new: &NewDraft, by: &Actor) -> CapResult<Draft> {
        self.check()?;
        self.inner.create_draft(new, by)
    }
    fn subscribe(&self) -> CapResult<Subscription<MailEvent>> {
        self.inner.subscribe()
    }
}
