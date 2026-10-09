//! A provider for the tests of the screen: a memory provider that can be told to fail a call, to leave a call out of
//! its capabilities, to page in small pages and to have fewer statuses.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use atelier_capabilities::{
    Actor, CapError, CapResult, Capabilities, Operation, Ref, Subscription,
    tasks::{
        Activity, Category, Comment, Event, Label, MemoryTasks, NewTask, Page, Patch, Project, Query, Status, Task, TasksProvider,
    },
};

#[derive(Default)]
struct Plan {
    errors: HashMap<Operation, CapError>,
    left_out: Vec<Operation>,
    page_max: Option<u32>,
    statuses: Option<Vec<Category>>,
}

pub struct Fake {
    inner: MemoryTasks,
    plan: Mutex<Plan>,
}

impl Fake {
    /// A provider of `account` that holds a task for each of `titles`.
    pub fn seeded(account: &str, titles: &[&str]) -> Arc<Self> {
        let inner = MemoryTasks::new(account);
        let me = Actor::person("me", "me");
        titles.iter().for_each(|title| drop(inner.create(&NewTask::titled(*title), &me).unwrap()));
        Arc::new(Self { inner, plan: Mutex::new(Plan::default()) })
    }

    pub fn memory(&self) -> &MemoryTasks {
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

    pub fn pages_of(&self, max: u32) {
        self.plan.lock().unwrap().page_max = Some(max);
    }

    /// Only these statuses.
    pub fn statuses_only(&self, categories: &[Category]) {
        self.plan.lock().unwrap().statuses = Some(categories.to_vec());
    }

    fn check(&self, operation: Operation) -> CapResult<()> {
        match self.plan.lock().unwrap().errors.get(&operation) {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

impl TasksProvider for Fake {
    fn provider(&self) -> &str {
        self.inner.provider()
    }

    fn account(&self) -> &str {
        self.inner.account()
    }

    fn capabilities(&self) -> Capabilities {
        let plan = self.plan.lock().unwrap();
        let mut caps = self.inner.capabilities();
        caps.operations.retain(|o| !plan.left_out.contains(o));
        if plan.page_max.is_some() {
            caps.limits.page_max = plan.page_max;
        }
        caps
    }

    fn whoami(&self) -> CapResult<Actor> {
        self.inner.whoami()
    }

    fn list(&self, query: &Query) -> CapResult<Page<Task>> {
        self.check(Operation::List)?;
        self.inner.list(query)
    }

    fn get(&self, task: &Ref) -> CapResult<Task> {
        self.inner.get(task)
    }

    fn create(&self, new: &NewTask, by: &Actor) -> CapResult<Task> {
        self.check(Operation::Create)?;
        self.inner.create(new, by)
    }

    fn update(&self, task: &Ref, patch: &Patch, version: &str, by: &Actor) -> CapResult<Task> {
        self.check(Operation::Update)?;
        self.inner.update(task, patch, version, by)
    }

    fn comment(&self, task: &Ref, body: &str, by: &Actor) -> CapResult<Comment> {
        self.check(Operation::Comment)?;
        self.inner.comment(task, body, by)
    }

    fn activity(&self, task: &Ref, cursor: Option<&str>) -> CapResult<Page<Activity>> {
        self.inner.activity(task, cursor)
    }

    fn labels(&self) -> CapResult<Vec<Label>> {
        self.check(Operation::Labels)?;
        self.inner.labels()
    }

    fn projects(&self) -> CapResult<Vec<Project>> {
        self.inner.projects()
    }

    fn subscribe(&self) -> CapResult<Subscription<Event>> {
        self.inner.subscribe()
    }

    fn statuses(&self) -> CapResult<Vec<Status>> {
        match &self.plan.lock().unwrap().statuses {
            Some(only) => Ok(only.iter().map(|c| Status::plain(*c)).collect()),
            None => self.inner.statuses(),
        }
    }
}
