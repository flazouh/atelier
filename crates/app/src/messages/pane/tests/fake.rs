//! A provider for the tests of the screen: a memory provider that can be told to fail a call, to leave a call or a feature
//! out of its capabilities, to keep no formatting, and that counts the calls it gets.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use atelier_capabilities::{
    Actor, CapError, CapResult, Ref, Subscription,
    messaging::{
        Channel, ChannelKind, ChannelQuery, Envelope, Event, Feature, Filter, Formatting,
        MemoryMessaging, Message, MessagingCapabilities, MessagingProvider, NewMessage, Operation,
        Page, SearchQuery, Workspace,
    },
};

#[derive(Default)]
struct Plan {
    errors: HashMap<Operation, CapError>,
    left_out: Vec<Operation>,
    features_off: Vec<Feature>,
    formatting: Option<Formatting>,
    calls: HashMap<Operation, usize>,
}

pub struct Fake {
    inner: MemoryMessaging,
    plan: Mutex<Plan>,
}

impl Fake {
    /// A provider of `account` with a public channel `general`, that holds a message for each of `texts`.
    pub fn seeded(account: &str, texts: &[&str]) -> (Arc<Self>, Ref) {
        Self::over(MemoryMessaging::new(account), texts)
    }

    /// The same, over a memory provider the test made (one with a smaller page, for one).
    pub fn over(inner: MemoryMessaging, texts: &[&str]) -> (Arc<Self>, Ref) {
        let channel = inner.add_channel("general", ChannelKind::Public);
        let me = inner.whoami().unwrap();
        texts
            .iter()
            .for_each(|text| drop(inner.send(&NewMessage::to(&channel, text), &me).unwrap()));
        (
            Arc::new(Self {
                inner,
                plan: Mutex::new(Plan::default()),
            }),
            channel,
        )
    }

    pub fn memory(&self) -> &MemoryMessaging {
        &self.inner
    }

    /// The call answers with `error` until it is healed.
    pub fn fail(&self, operation: Operation, error: CapError) {
        self.plan.lock().unwrap().errors.insert(operation, error);
    }

    pub fn heal(&self, operation: Operation) {
        self.plan.lock().unwrap().errors.remove(&operation);
    }

    /// The call is not in the capabilities.
    pub fn without(&self, operation: Operation) {
        self.plan.lock().unwrap().left_out.push(operation);
    }

    /// The service has no such idea.
    pub fn without_feature(&self, feature: Feature) {
        self.plan.lock().unwrap().features_off.push(feature);
    }

    pub fn keeping(&self, formatting: Formatting) {
        self.plan.lock().unwrap().formatting = Some(formatting);
    }

    /// How many times the screen asked.
    pub fn calls(&self, operation: Operation) -> usize {
        self.plan
            .lock()
            .unwrap()
            .calls
            .get(&operation)
            .copied()
            .unwrap_or(0)
    }

    fn check(&self, operation: Operation) -> CapResult<()> {
        let mut plan = self.plan.lock().unwrap();
        *plan.calls.entry(operation).or_default() += 1;
        match plan.errors.get(&operation) {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

impl MessagingProvider for Fake {
    fn provider(&self) -> &str {
        self.inner.provider()
    }

    fn account(&self) -> &str {
        self.inner.account()
    }

    fn capabilities(&self) -> MessagingCapabilities {
        let plan = self.plan.lock().unwrap();
        let mut caps = self.inner.capabilities();
        caps.operations.retain(|o| !plan.left_out.contains(o));
        caps.features.retain(|f| !plan.features_off.contains(f));
        if let Some(formatting) = plan.formatting {
            caps.formatting = formatting;
        }
        caps
    }

    fn whoami(&self) -> CapResult<Actor> {
        self.inner.whoami()
    }

    fn workspace(&self) -> CapResult<Workspace> {
        self.inner.workspace()
    }

    fn channels(&self, query: &ChannelQuery) -> CapResult<Page<Channel>> {
        self.check(Operation::Channels)?;
        self.inner.channels(query)
    }

    fn history(
        &self,
        channel: &Ref,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> CapResult<Page<Message>> {
        self.check(Operation::History)?;
        self.inner.history(channel, cursor, limit)
    }

    fn thread(&self, root: &Ref, cursor: Option<&str>) -> CapResult<Page<Message>> {
        self.check(Operation::Thread)?;
        self.inner.thread(root, cursor)
    }

    fn send(&self, new: &NewMessage, by: &Actor) -> CapResult<Message> {
        self.check(Operation::Send)?;
        self.inner.send(new, by)
    }

    fn subscribe(&self, filter: &Filter) -> CapResult<Subscription<Event>> {
        self.check(Operation::Subscribe)?;
        self.inner.subscribe(filter)
    }

    fn search(&self, query: &SearchQuery) -> CapResult<Page<Message>> {
        self.inner.search(query)
    }

    fn mark_read(&self, channel: &Ref, up_to: Option<&Ref>) -> CapResult<()> {
        self.check(Operation::MarkRead)?;
        self.inner.mark_read(channel, up_to)
    }

    fn export(&self, cursor: Option<&str>) -> CapResult<Page<Envelope>> {
        self.inner.export(cursor)
    }
}
