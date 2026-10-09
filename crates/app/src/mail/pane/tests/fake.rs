//! A provider for the tests of the screen: a memory provider that can be told to fail a call, to leave a call out of its
//! capabilities, to mark a thread read when it gives it (as Gmail does), to have its draft changed behind the screen's back, and
//! that counts the calls it gets.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use atelier_capabilities::{
    Actor, CapError, CapResult, Ref, Subscription,
    mail::{
        Account, Approval, Draft, DraftPatch, Incoming, MailCapabilities, MailEvent, MailOperation,
        MailProvider, Mailbox, MemoryMail, Message, NewDraft, SearchQuery, Thread, ThreadSummary,
    },
    tasks::Page,
};

#[derive(Default)]
struct Plan {
    errors: HashMap<MailOperation, CapError>,
    left_out: Vec<MailOperation>,
    calls: HashMap<MailOperation, usize>,
    marks_read_on_read: bool,
    tamper_before_send: bool,
    page_max: Option<u32>,
    queries: Vec<SearchQuery>,
    sent: Vec<Sent>,
}

/// A call of `send`: the draft, the version the screen named, and what the provider held under that draft at that moment.
#[derive(Clone, Debug)]
pub struct Sent {
    pub draft: Ref,
    pub version: String,
    pub text: String,
    pub held_version: String,
}

pub struct Fake {
    inner: MemoryMail,
    plan: Mutex<Plan>,
}

/// The first message's date; each next one comes a minute later, so the newest is the last one delivered.
const BASE: i64 = 1_790_000_000_000;

impl Fake {
    /// A provider of `address` that holds one thread in its inbox for each of `subjects`, the last one the newest.
    pub fn seeded(address: &str, subjects: &[&str]) -> Arc<Self> {
        let inner = MemoryMail::new(address);
        for (n, subject) in subjects.iter().enumerate() {
            inner
                .receive(&Incoming::new(
                    "ana@example.com",
                    address,
                    subject,
                    &format!("The body of {subject}"),
                    BASE + n as i64 * 60_000,
                ))
                .unwrap();
        }
        Arc::new(Self {
            inner,
            plan: Mutex::new(Plan::default()),
        })
    }

    pub fn memory(&self) -> &MemoryMail {
        &self.inner
    }

    /// The call answers with `error` until it is healed.
    pub fn fail(&self, operation: MailOperation, error: CapError) {
        self.plan.lock().unwrap().errors.insert(operation, error);
    }

    pub fn heal(&self, operation: MailOperation) {
        self.plan.lock().unwrap().errors.remove(&operation);
    }

    /// The call is not in the capabilities.
    pub fn without(&self, operation: MailOperation) {
        self.plan.lock().unwrap().left_out.push(operation);
    }

    /// The provider lists no call that writes, as Gmail does today.
    pub fn read_only(&self) {
        for operation in MailOperation::ALL {
            if !matches!(
                operation,
                MailOperation::Mailboxes
                    | MailOperation::Search
                    | MailOperation::Thread
                    | MailOperation::Get
                    | MailOperation::DownloadAttachment
                    | MailOperation::Subscribe
            ) {
                self.without(operation);
            }
        }
    }

    /// Giving a thread marks it read, as Gmail does.
    pub fn marks_read_on_read(&self) {
        self.plan.lock().unwrap().marks_read_on_read = true;
    }

    /// The draft is changed by someone else just before it is sent.
    pub fn tamper_before_send(&self) {
        self.plan.lock().unwrap().tamper_before_send = true;
    }

    pub fn page_max(&self, page: u32) {
        self.plan.lock().unwrap().page_max = Some(page);
    }

    /// How many times the screen asked.
    pub fn calls(&self, operation: MailOperation) -> usize {
        self.plan
            .lock()
            .unwrap()
            .calls
            .get(&operation)
            .copied()
            .unwrap_or(0)
    }

    /// Every search the screen made.
    pub fn queries(&self) -> Vec<SearchQuery> {
        self.plan.lock().unwrap().queries.clone()
    }

    /// The drafts and versions `send` was called with.
    pub fn sent(&self) -> Vec<Sent> {
        self.plan.lock().unwrap().sent.clone()
    }

    fn check(&self, operation: MailOperation) -> CapResult<()> {
        let mut plan = self.plan.lock().unwrap();
        *plan.calls.entry(operation).or_default() += 1;
        match plan.errors.get(&operation) {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

impl MailProvider for Fake {
    fn provider(&self) -> &str {
        self.inner.provider()
    }

    fn account(&self) -> &str {
        self.inner.account()
    }

    fn capabilities(&self) -> MailCapabilities {
        let plan = self.plan.lock().unwrap();
        let mut caps = self.inner.capabilities();
        caps.operations.retain(|o| !plan.left_out.contains(o));
        if let Some(page) = plan.page_max {
            caps.limits.page_max = Some(page);
        }
        caps
    }

    fn whoami(&self) -> CapResult<Account> {
        self.inner.whoami()
    }

    fn mailboxes(&self) -> CapResult<Vec<Mailbox>> {
        self.check(MailOperation::Mailboxes)?;
        self.inner.mailboxes()
    }

    fn search(&self, query: &SearchQuery) -> CapResult<Page<ThreadSummary>> {
        self.check(MailOperation::Search)?;
        self.plan.lock().unwrap().queries.push(query.clone());
        self.inner.search(query)
    }

    fn thread(&self, thread: &Ref) -> CapResult<Thread> {
        self.check(MailOperation::Thread)?;
        if self.plan.lock().unwrap().marks_read_on_read {
            self.inner
                .mark_read(thread, true, &Actor::person("gmail", "gmail"))?;
        }
        self.inner.thread(thread)
    }

    fn get(&self, message: &Ref) -> CapResult<Message> {
        self.check(MailOperation::Get)?;
        self.inner.get(message)
    }

    fn draft(&self, draft: &Ref) -> CapResult<Draft> {
        self.inner.draft(draft)
    }

    fn mark_read(&self, target: &Ref, read: bool, by: &Actor) -> CapResult<()> {
        self.check(MailOperation::MarkRead)?;
        self.inner.mark_read(target, read, by)
    }

    fn star(&self, target: &Ref, starred: bool, by: &Actor) -> CapResult<()> {
        self.check(MailOperation::Star)?;
        self.inner.star(target, starred, by)
    }

    fn archive(&self, target: &Ref, by: &Actor) -> CapResult<()> {
        self.check(MailOperation::Archive)?;
        self.inner.archive(target, by)
    }

    fn label(&self, target: &Ref, add: &[Ref], remove: &[Ref], by: &Actor) -> CapResult<()> {
        self.check(MailOperation::Label)?;
        self.inner.label(target, add, remove, by)
    }

    fn move_to(&self, target: &Ref, mailbox: &Ref, by: &Actor) -> CapResult<()> {
        self.check(MailOperation::Move)?;
        self.inner.move_to(target, mailbox, by)
    }

    fn trash(&self, target: &Ref, by: &Actor) -> CapResult<()> {
        self.check(MailOperation::Trash)?;
        self.inner.trash(target, by)
    }

    fn create_draft(&self, new: &NewDraft, by: &Actor) -> CapResult<Draft> {
        self.check(MailOperation::CreateDraft)?;
        self.inner.create_draft(new, by)
    }

    fn update_draft(
        &self,
        draft: &Ref,
        patch: &DraftPatch,
        version: &str,
        by: &Actor,
    ) -> CapResult<Draft> {
        self.check(MailOperation::UpdateDraft)?;
        self.inner.update_draft(draft, patch, version, by)
    }

    fn send(
        &self,
        draft: &Ref,
        version: &str,
        by: &Actor,
        approval: Option<&Approval>,
    ) -> CapResult<Message> {
        // The try is counted before it may fail: the screen asked, whatever the provider answers.
        if let Ok(held) = self.inner.draft(draft) {
            self.plan.lock().unwrap().sent.push(Sent {
                draft: draft.clone(),
                version: version.to_string(),
                text: held.text,
                held_version: held.version,
            });
        }
        self.check(MailOperation::Send)?;
        if self.plan.lock().unwrap().tamper_before_send {
            let now = self.inner.draft(draft)?;
            self.inner.update_draft(
                draft,
                &DraftPatch {
                    text: Some(format!("{} (changed elsewhere)", now.text)),
                    ..DraftPatch::default()
                },
                &now.version,
                &Actor::agent("claude", "Claude", "me"),
            )?;
        }
        self.inner.send(draft, version, by, approval)
    }

    fn download_attachment(&self, attachment: &Ref) -> CapResult<Vec<u8>> {
        self.inner.download_attachment(attachment)
    }

    fn subscribe(&self) -> CapResult<Subscription<MailEvent>> {
        self.check(MailOperation::Subscribe)?;
        self.inner.subscribe()
    }
}
