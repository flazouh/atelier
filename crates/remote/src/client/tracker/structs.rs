use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc::{Sender, channel}},
    thread,
    time::Duration,
};

use atelier_tracker::{
    Activity, Entry, Event, NewTask, Patch, Query, StopFlag, Subscription, Task, TaskId,
    Tracker, TrackerResult,
};

use super::super::{Shared, lock};
use crate::protocol::tracker::{TrackerCall, TrackerReply};
use super::helpers::{ask, poll, unexpected};

/// One receiver, and what it knows: each task's last change. `None` until its first listing.
pub(super) struct Listener {
    pub(super) send: Sender<Event>,
    pub(super) stop: StopFlag,
    pub(super) seen: Option<HashMap<TaskId, i64>>,
}

#[derive(Default)]
pub(super) struct Listeners {
    pub(super) all: Vec<Listener>,
    pub(super) polling: bool,
}

pub struct RemoteTracker {
    pub(super) shared: Arc<Shared>,
    name: String,
    pub(super) poll: Duration,
    pub(super) listeners: Arc<Mutex<Listeners>>,
}

impl RemoteTracker {
    /// Asks the host to open the project's store, and keeps its name.
    pub(in super::super) fn open(shared: Arc<Shared>, poll: Duration) -> TrackerResult<Self> {
        let TrackerReply::Name(name) = ask(&shared, TrackerCall::Name)? else { return Err(unexpected()) };
        Ok(Self { shared, name, poll, listeners: Arc::default() })
    }

    pub(super) fn ask(&self, call: TrackerCall) -> TrackerResult<TrackerReply> {
        ask(&self.shared, call)
    }

    /// Tells each receiver of a change made through this tracker, at once, as the local tracker does.
    fn tell(&self, event: Event) {
        let mut listeners = lock(&self.listeners);
        if let Event::Created(task) | Event::Updated(task) = &event {
            for listener in &mut listeners.all {
                if let Some(seen) = &mut listener.seen {
                    seen.insert(task.id.clone(), task.updated_at);
                }
            }
        }
        listeners.all.retain(|l| !l.stop.is_stopped() && l.send.send(event.clone()).is_ok());
    }
}

impl Tracker for RemoteTracker {
    fn name(&self) -> &str {
        &self.name
    }

    fn list(&self, query: &Query) -> TrackerResult<Vec<Task>> {
        match self.ask(TrackerCall::List { query: query.clone() })? {
            TrackerReply::Tasks(tasks) => Ok(tasks),
            _ => Err(unexpected()),
        }
    }

    fn get(&self, id: &TaskId) -> TrackerResult<Option<Task>> {
        match self.ask(TrackerCall::Get { id: id.clone() })? {
            TrackerReply::Found(task) => Ok(task),
            _ => Err(unexpected()),
        }
    }

    fn create(&self, new: &NewTask, by: &str) -> TrackerResult<Task> {
        match self.ask(TrackerCall::Create { new: new.clone(), by: by.into() })? {
            TrackerReply::Task(task) => {
                self.tell(Event::Created(task.clone()));
                Ok(task)
            }
            _ => Err(unexpected()),
        }
    }

    fn create_many(&self, new: &[NewTask], by: &str) -> TrackerResult<Vec<Task>> {
        match self.ask(TrackerCall::CreateMany { new: new.to_vec(), by: by.into() })? {
            TrackerReply::Tasks(tasks) => {
                for task in &tasks {
                    self.tell(Event::Created(task.clone()));
                }
                Ok(tasks)
            }
            _ => Err(unexpected()),
        }
    }

    fn update(&self, id: &TaskId, patch: &Patch, by: &str) -> TrackerResult<Task> {
        match self.ask(TrackerCall::Update { id: id.clone(), patch: patch.clone(), by: by.into() })? {
            TrackerReply::Task(task) => {
                self.tell(Event::Updated(task.clone()));
                Ok(task)
            }
            _ => Err(unexpected()),
        }
    }

    fn record(&self, id: &TaskId, entry: &Entry, by: &str) -> TrackerResult<Activity> {
        match self.ask(TrackerCall::Record { id: id.clone(), entry: entry.clone(), by: by.into() })? {
            TrackerReply::Activity(activity) => {
                self.tell(Event::Activity(activity.clone()));
                Ok(activity)
            }
            _ => Err(unexpected()),
        }
    }

    fn activity(&self, id: &TaskId) -> TrackerResult<Vec<Activity>> {
        match self.ask(TrackerCall::Activity { id: id.clone() })? {
            TrackerReply::Log(log) => Ok(log),
            _ => Err(unexpected()),
        }
    }

    fn tasks_of_session(&self, session_id: &str) -> TrackerResult<Vec<TaskId>> {
        match self.ask(TrackerCall::TasksOfSession { session_id: session_id.into() })? {
            TrackerReply::Ids(ids) => Ok(ids),
            _ => Err(unexpected()),
        }
    }

    fn tasks_of_pr(&self, number: u64) -> TrackerResult<Vec<TaskId>> {
        match self.ask(TrackerCall::TasksOfPr { number })? {
            TrackerReply::Ids(ids) => Ok(ids),
            _ => Err(unexpected()),
        }
    }

    fn labels(&self) -> TrackerResult<Vec<String>> {
        match self.ask(TrackerCall::Labels)? {
            TrackerReply::Labels(labels) => Ok(labels),
            _ => Err(unexpected()),
        }
    }

    /// Changes made through this tracker at once, and changes made elsewhere by a poll of the host every
    /// [`POLL`](super::POLL). It blocks for one listing, the start the poll compares with. The poll runs while a
    /// receiver is kept: drop it when the tasks are out of sight.
    fn subscribe(&self) -> Subscription {
        let (send, receive) = channel();
        let (subscription, stop) = Subscription::new(receive);
        // What the host has now, before this returns: a change made after it is told, never taken as the
        // start. A failed listing leaves it to the first poll.
        let seen = match self.list(&Query::default()) {
            Ok(tasks) => Some(tasks.iter().map(|t| (t.id.clone(), t.updated_at)).collect()),
            Err(_) => None,
        };
        let mut listeners = lock(&self.listeners);
        listeners.all.retain(|l| !l.stop.is_stopped());
        listeners.all.push(Listener { send, stop, seen });
        if !listeners.polling {
            listeners.polling = true;
            let (shared, list, every) = (Arc::downgrade(&self.shared), self.listeners.clone(), self.poll);
            thread::spawn(move || poll(shared, list, every));
        }
        subscription
    }
}
