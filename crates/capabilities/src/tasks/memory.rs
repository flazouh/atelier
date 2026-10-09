use std::{
    sync::{Mutex, mpsc::{Sender, channel}},
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    structs::{Activity, Comment, Envelope, Event, Label, NewTask, Page, Patch, Project, Query, Status, Task},
    traits::TasksProvider,
    types::{ActivityKind, Category, Change, EntityKind, EventKind, Priority, Sort},
};
use crate::{Actor, AuthKind, CapError, CapResult, Capabilities, Limits, Operation, Ref, StopFlag, Subscription};

const PAGE_DEFAULT: usize = 50;
const PAGE_MAX: usize = 100;

const CATEGORIES: [Category; 6] = [Category::Backlog, Category::Todo, Category::InProgress, Category::InReview, Category::Done, Category::Canceled];

/// A tasks provider that keeps everything in memory. It is the reference for the contract suite, and a stand-in for tests
/// of the screen and the agent tools. It lists every optional call except `delete`.
pub struct MemoryTasks {
    account: String,
    me: Actor,
    clock: Box<dyn Fn() -> i64 + Send + Sync>,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    tasks: Vec<Task>,
    comments: Vec<Comment>,
    activity: Vec<Activity>,
    actors: Vec<Actor>,
    counter: u64,
    subscribers: Vec<(Sender<Event>, StopFlag)>,
}

fn system_clock() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

impl MemoryTasks {
    pub fn new(account: &str) -> Self {
        let me = Actor::person("me", "Me");
        Self {
            account: account.to_string(),
            inner: Mutex::new(Inner { actors: vec![me.clone()], ..Inner::default() }),
            me,
            clock: Box::new(system_clock),
        }
    }

    /// Reads time from `clock`, in milliseconds, so a test moves it.
    pub fn with_clock(mut self, clock: impl Fn() -> i64 + Send + Sync + 'static) -> Self {
        self.clock = Box::new(clock);
        self
    }

    fn make_ref(&self, id: &str) -> Ref {
        Ref { capability: "tasks".into(), provider: "memory".into(), account: self.account.clone(), id: id.to_string() }
    }

    fn activity_ref(&self, n: u64) -> Ref {
        self.make_ref(&format!("activity-{n}"))
    }

    fn lock(&self) -> CapResult<std::sync::MutexGuard<'_, Inner>> {
        self.inner.lock().map_err(|_| CapError::Storage { message: "the memory store is poisoned".into() })
    }

    fn status_of(&self, id: &str) -> CapResult<Status> {
        CATEGORIES.iter().map(|c| Status::plain(*c)).find(|s| s.id == id).ok_or_else(|| CapError::invalid("status"))
    }

    fn actors_of(inner: &Inner, ids: &[String]) -> CapResult<Vec<Actor>> {
        ids.iter().map(|id| inner.actors.iter().find(|a| &a.id == id).cloned().ok_or_else(|| CapError::invalid("assignees"))).collect()
    }

    fn remember(inner: &mut Inner, by: &Actor) {
        if !inner.actors.iter().any(|a| a.id == by.id) {
            inner.actors.push(by.clone());
        }
    }

    fn log(&self, inner: &mut Inner, task: &Ref, by: &Actor, kind: ActivityKind, detail: Option<serde_json::Value>) -> Activity {
        inner.counter += 1;
        let activity = Activity { reference: self.activity_ref(inner.counter), task: task.clone(), at: (self.clock)(), by: by.clone(), kind, detail };
        inner.activity.push(activity.clone());
        activity
    }

    fn tell(inner: &mut Inner, event: Event) {
        inner.subscribers.retain(|(tx, stop)| !stop.is_stopped() && tx.send(event.clone()).is_ok());
    }

    fn bump(inner: &mut Inner) -> String {
        inner.counter += 1;
        inner.counter.to_string()
    }
}

impl TasksProvider for MemoryTasks {
    fn provider(&self) -> &str {
        "memory"
    }

    fn account(&self) -> &str {
        &self.account
    }

    fn capabilities(&self) -> Capabilities {
        let mut operations = Capabilities::CORE.to_vec();
        operations.extend([Operation::Statuses, Operation::Export, Operation::Import]);
        Capabilities { operations, features: vec![], limits: Limits { page_max: Some(PAGE_MAX as u32), per_minute: None }, auth: vec![AuthKind::None] }
    }

    fn whoami(&self) -> CapResult<Actor> {
        Ok(self.me.clone())
    }

    fn list(&self, query: &Query) -> CapResult<Page<Task>> {
        let inner = self.lock()?;
        let mut found: Vec<&Task> = inner
            .tasks
            .iter()
            .filter(|t| query.status.is_empty() || query.status.contains(&t.status.category))
            .filter(|t| query.assignee.as_ref().is_none_or(|id| t.assignees.iter().any(|a| &a.id == id)))
            .filter(|t| query.project.as_ref().is_none_or(|p| t.project.as_ref() == Some(p)))
            .filter(|t| query.labels.iter().all(|l| t.labels.contains(l)))
            .filter(|t| query.text.as_ref().is_none_or(|text| {
                let text = text.to_lowercase();
                t.title.to_lowercase().contains(&text) || t.description.to_lowercase().contains(&text)
            }))
            .filter(|t| query.linked_to.as_ref().is_none_or(|to| t.links.iter().any(|l| &l.reference == to)))
            .collect();
        match query.sort {
            Sort::Updated => found.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then_with(|| b.key.cmp(&a.key))),
            Sort::Created => found.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| b.key.cmp(&a.key))),
            Sort::Priority => found.sort_by_key(|t| (if t.priority == Priority::None { 9 } else { t.priority.number() }, t.key.clone())),
        }
        let start = match &query.cursor {
            Some(cursor) => cursor.parse::<usize>().map_err(|_| CapError::invalid("cursor"))?,
            None => 0,
        };
        let size = query.limit.map_or(PAGE_DEFAULT, |n| (n as usize).clamp(1, PAGE_MAX));
        let end = (start + size).min(found.len());
        let items = found.get(start..end).unwrap_or_default().iter().map(|t| (*t).clone()).collect();
        Ok(Page { items, next_cursor: (end < found.len()).then(|| end.to_string()) })
    }

    fn get(&self, task: &Ref) -> CapResult<Task> {
        self.lock()?.tasks.iter().find(|t| &t.reference == task).cloned().ok_or_else(|| CapError::not_found(task.to_string()))
    }

    fn create(&self, new: &NewTask, by: &Actor) -> CapResult<Task> {
        let title = new.title.trim();
        if title.is_empty() {
            return Err(CapError::invalid("title"));
        }
        let status = match &new.status {
            Some(id) => self.status_of(id)?,
            None => Status::plain(Category::Todo),
        };
        let mut inner = self.lock()?;
        Self::remember(&mut inner, by);
        let assignees = Self::actors_of(&inner, &new.assignees)?;
        let n = inner.tasks.len() + 1;
        let key = format!("MEM-{n}");
        let now = (self.clock)();
        let task = Task {
            reference: self.make_ref(&key),
            key,
            title: title.to_string(),
            description: new.description.clone(),
            status,
            priority: new.priority,
            assignees,
            labels: new.labels.clone(),
            project: new.project.clone(),
            parent: new.parent.clone(),
            links: vec![],
            created_at: now,
            updated_at: now,
            version: Self::bump(&mut inner),
            due_at: None,
            estimate: None,
            raw: None,
        };
        inner.tasks.push(task.clone());
        let activity = self.log(&mut inner, &task.reference, by, ActivityKind::Created, None);
        Self::tell(&mut inner, Event { kind: EventKind::Created, task: task.clone(), activity: Some(activity) });
        Ok(task)
    }

    fn update(&self, task: &Ref, patch: &Patch, version: &str, by: &Actor) -> CapResult<Task> {
        let status = patch.status.as_deref().map(|id| self.status_of(id)).transpose()?;
        let mut inner = self.lock()?;
        Self::remember(&mut inner, by);
        let at = inner.tasks.iter().position(|t| &t.reference == task).ok_or_else(|| CapError::not_found(task.to_string()))?;
        if inner.tasks[at].version != version {
            return Err(CapError::Conflict { current: serde_json::to_value(&inner.tasks[at]).unwrap_or_default() });
        }
        let assignees = patch.assignees.as_ref().map(|ids| Self::actors_of(&inner, ids)).transpose()?;
        if let Some(title) = &patch.title
            && title.trim().is_empty()
        {
            return Err(CapError::invalid("title"));
        }
        let before = inner.tasks[at].clone();
        let mut next = before.clone();
        if let Some(title) = &patch.title {
            next.title = title.trim().to_string();
        }
        match &patch.description {
            Change::Keep => {}
            Change::Set(text) => next.description = text.clone(),
            Change::Clear => next.description.clear(),
        }
        if let Some(status) = status {
            next.status = status;
        }
        if let Some(priority) = patch.priority {
            next.priority = priority;
        }
        match &patch.project {
            Change::Keep => {}
            Change::Set(p) => next.project = Some(p.clone()),
            Change::Clear => next.project = None,
        }
        if let Some(labels) = &patch.labels {
            next.labels = labels.clone();
        }
        if let Some(assignees) = assignees {
            next.assignees = assignees;
        }
        match &patch.parent {
            Change::Keep => {}
            Change::Set(p) => next.parent = Some(p.clone()),
            Change::Clear => next.parent = None,
        }
        if next == before {
            return Ok(before);
        }
        next.updated_at = (self.clock)();
        next.version = Self::bump(&mut inner);
        inner.tasks[at] = next.clone();
        let mut last = None;
        if before.status != next.status {
            last = Some(self.log(&mut inner, task, by, ActivityKind::StatusChanged, Some(serde_json::json!({ "from": before.status.id, "to": next.status.id }))));
        }
        if before.assignees != next.assignees {
            last = Some(self.log(&mut inner, task, by, ActivityKind::Assigned, None));
        }
        let edited = Task { status: before.status.clone(), assignees: before.assignees.clone(), updated_at: before.updated_at, version: before.version.clone(), ..next.clone() } != before;
        if edited {
            last = Some(self.log(&mut inner, task, by, ActivityKind::Edited, None));
        }
        Self::tell(&mut inner, Event { kind: EventKind::Updated, task: next.clone(), activity: last });
        Ok(next)
    }

    fn comment(&self, task: &Ref, body: &str, by: &Actor) -> CapResult<Comment> {
        if body.trim().is_empty() {
            return Err(CapError::invalid("body"));
        }
        let mut inner = self.lock()?;
        let current = inner.tasks.iter().find(|t| &t.reference == task).cloned().ok_or_else(|| CapError::not_found(task.to_string()))?;
        Self::remember(&mut inner, by);
        inner.counter += 1;
        let comment = Comment {
            reference: self.make_ref(&format!("comment-{}", inner.counter)),
            task: task.clone(),
            author: by.clone(),
            body: body.to_string(),
            created_at: (self.clock)(),
            updated_at: None,
            raw: None,
        };
        inner.comments.push(comment.clone());
        let activity = self.log(&mut inner, task, by, ActivityKind::Commented, None);
        Self::tell(&mut inner, Event { kind: EventKind::Activity, task: current, activity: Some(activity) });
        Ok(comment)
    }

    fn activity(&self, task: &Ref, cursor: Option<&str>) -> CapResult<Page<Activity>> {
        let inner = self.lock()?;
        if !inner.tasks.iter().any(|t| &t.reference == task) {
            return Err(CapError::not_found(task.to_string()));
        }
        let all: Vec<&Activity> = inner.activity.iter().filter(|a| &a.task == task).collect();
        let start = match cursor {
            Some(c) => c.parse::<usize>().map_err(|_| CapError::invalid("cursor"))?,
            None => 0,
        };
        let end = (start + PAGE_DEFAULT).min(all.len());
        Ok(Page { items: all.get(start..end).unwrap_or_default().iter().map(|a| (*a).clone()).collect(), next_cursor: (end < all.len()).then(|| end.to_string()) })
    }

    fn labels(&self) -> CapResult<Vec<Label>> {
        let inner = self.lock()?;
        let mut refs: Vec<&Ref> = inner.tasks.iter().flat_map(|t| t.labels.iter()).collect();
        refs.sort();
        refs.dedup();
        Ok(refs.into_iter().map(|r| Label { reference: r.clone(), name: r.id.clone(), color: None, raw: None }).collect())
    }

    fn projects(&self) -> CapResult<Vec<Project>> {
        let inner = self.lock()?;
        let mut refs: Vec<&Ref> = inner.tasks.iter().filter_map(|t| t.project.as_ref()).collect();
        refs.sort();
        refs.dedup();
        Ok(refs.into_iter().map(|r| Project { reference: r.clone(), key: r.id.clone(), name: r.id.clone(), raw: None }).collect())
    }

    fn subscribe(&self) -> CapResult<Subscription<Event>> {
        let (tx, rx) = channel();
        let (subscription, stop) = Subscription::new(rx);
        self.lock()?.subscribers.push((tx, stop));
        Ok(subscription)
    }

    fn statuses(&self) -> CapResult<Vec<Status>> {
        Ok(CATEGORIES.iter().map(|c| Status::plain(*c)).collect())
    }

    fn export(&self, cursor: Option<&str>) -> CapResult<Page<Envelope>> {
        let inner = self.lock()?;
        let mut all: Vec<Envelope> = Vec::new();
        let envelope = |kind: EntityKind, value: serde_json::Value| Envelope { kind, entity: value, raw: None };
        all.extend(inner.tasks.iter().map(|t| envelope(EntityKind::Task, serde_json::to_value(t).unwrap_or_default())));
        all.extend(inner.comments.iter().map(|c| envelope(EntityKind::Comment, serde_json::to_value(c).unwrap_or_default())));
        all.extend(inner.activity.iter().map(|a| envelope(EntityKind::Activity, serde_json::to_value(a).unwrap_or_default())));
        let start = match cursor {
            Some(c) => c.parse::<usize>().map_err(|_| CapError::invalid("cursor"))?,
            None => 0,
        };
        let end = (start + PAGE_DEFAULT).min(all.len());
        let items = all.get(start..end).unwrap_or_default().to_vec();
        Ok(Page { items, next_cursor: (end < all.len()).then(|| end.to_string()) })
    }

    fn import(&self, batch: &[Envelope]) -> CapResult<usize> {
        let mut inner = self.lock()?;
        let mut written = 0;
        for envelope in batch {
            let invalid = |_| CapError::invalid("entity");
            match envelope.kind {
                EntityKind::Task => {
                    let task: Task = serde_json::from_value(envelope.entity.clone()).map_err(invalid)?;
                    match inner.tasks.iter().position(|t| t.reference == task.reference) {
                        Some(at) if inner.tasks[at] == task => {}
                        Some(at) => {
                            inner.tasks[at] = task;
                            written += 1;
                        }
                        None => {
                            inner.tasks.push(task);
                            written += 1;
                        }
                    }
                }
                EntityKind::Comment => {
                    let comment: Comment = serde_json::from_value(envelope.entity.clone()).map_err(invalid)?;
                    Self::remember(&mut inner, &comment.author);
                    if !inner.comments.iter().any(|c| c.reference == comment.reference) {
                        inner.comments.push(comment);
                        written += 1;
                    }
                }
                EntityKind::Activity => {
                    let activity: Activity = serde_json::from_value(envelope.entity.clone()).map_err(invalid)?;
                    Self::remember(&mut inner, &activity.by);
                    if !inner.activity.iter().any(|a| a.reference == activity.reference) {
                        inner.activity.push(activity);
                        written += 1;
                    }
                }
                EntityKind::Label | EntityKind::Project | EntityKind::Actor => {}
            }
        }
        let highest = inner.tasks.iter().filter_map(|t| t.version.parse::<u64>().ok()).max().unwrap_or(0);
        inner.counter = inner.counter.max(highest);
        Ok(written)
    }
}
