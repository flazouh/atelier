//! The host's tracker, over the connection: each call of the `Tracker` trait is one request. The store is
//! the host's own file, so a change made on another machine reaches `subscribe` only by a poll of `list`,
//! every [`POLL`], while a receiver is kept.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, Weak,
        mpsc::{Receiver, Sender, channel},
    },
    thread,
    time::Duration,
};

use lathe_tracker::{
    Activity, Entry, Event, NewTask, Patch, Query, Task, TaskId, Tracker, TrackerError, TrackerResult,
};

use super::{Shared, lock};
use crate::protocol::{
    Call, Reply,
    tracker::{TrackerCall, TrackerReply},
};

/// How often a listened-to tracker asks the host for changes made elsewhere.
pub const POLL: Duration = Duration::from_secs(3);

/// One receiver, and what it knows: each task's last change. `None` until its first listing.
struct Listener {
    send: Sender<Event>,
    seen: Option<HashMap<TaskId, i64>>,
}

#[derive(Default)]
struct Listeners {
    all: Vec<Listener>,
    polling: bool,
}

pub struct RemoteTracker {
    shared: Arc<Shared>,
    name: String,
    poll: Duration,
    listeners: Arc<Mutex<Listeners>>,
}

impl RemoteTracker {
    /// Asks the host to open the project's store, and keeps its name.
    pub(super) fn open(shared: Arc<Shared>, poll: Duration) -> TrackerResult<Self> {
        let TrackerReply::Name(name) = ask(&shared, TrackerCall::Name)? else { return Err(unexpected()) };
        Ok(Self { shared, name, poll, listeners: Arc::default() })
    }

    fn ask(&self, call: TrackerCall) -> TrackerResult<TrackerReply> {
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
        listeners.all.retain(|l| l.send.send(event.clone()).is_ok());
    }
}

fn ask(shared: &Shared, call: TrackerCall) -> TrackerResult<TrackerReply> {
    match shared.request(Call::Tracker(call)) {
        Ok(Reply::Tracker(answer)) => *answer,
        Ok(_) => Err(unexpected()),
        Err(error) => Err(TrackerError::Storage(error.to_string())),
    }
}

fn unexpected() -> TrackerError {
    TrackerError::Storage("the host gave an unexpected answer".into())
}

/// What changed since `seen`: a task it had not seen is new, one with another change time is updated.
fn changes(seen: &mut HashMap<TaskId, i64>, tasks: &[Task]) -> Vec<Event> {
    let mut events = Vec::new();
    for task in tasks {
        match seen.insert(task.id.clone(), task.updated_at) {
            None => events.push(Event::Created(task.clone())),
            Some(at) if at != task.updated_at => events.push(Event::Updated(task.clone())),
            Some(_) => {}
        }
    }
    events
}

/// Lists the host's tasks every `every` and tells each receiver what changed, until no receiver is left
/// or the connection is gone. A receiver that was dropped is found at the next change it is told of.
fn poll(shared: Weak<Shared>, listeners: Arc<Mutex<Listeners>>, every: Duration) {
    loop {
        let Some(shared) = shared.upgrade() else { break };
        let listed = ask(&shared, TrackerCall::List { query: Query::default() });
        drop(shared);
        {
            let mut listeners = lock(&listeners);
            // A failed listing (the link is down) waits for the next turn.
            if let Ok(TrackerReply::Tasks(tasks)) = &listed {
                listeners.all.retain_mut(|listener| match &mut listener.seen {
                    None => {
                        listener.seen = Some(tasks.iter().map(|t| (t.id.clone(), t.updated_at)).collect());
                        true
                    }
                    Some(seen) => changes(seen, tasks).into_iter().all(|event| listener.send.send(event).is_ok()),
                });
            }
            if listeners.all.is_empty() {
                listeners.polling = false;
                return;
            }
        }
        thread::sleep(every);
    }
    lock(&listeners).polling = false;
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
    /// [`POLL`]. It blocks for one listing, the start the poll compares with. The poll runs while a
    /// receiver is kept: drop it when the tasks are out of sight.
    fn subscribe(&self) -> Receiver<Event> {
        let (send, receive) = channel();
        // What the host has now, before this returns: a change made after it is told, never taken as the
        // start. A failed listing leaves it to the first poll.
        let seen = match self.list(&Query::default()) {
            Ok(tasks) => Some(tasks.iter().map(|t| (t.id.clone(), t.updated_at)).collect()),
            Err(_) => None,
        };
        let mut listeners = lock(&self.listeners);
        listeners.all.push(Listener { send, seen });
        if !listeners.polling {
            listeners.polling = true;
            let (shared, list, every) = (Arc::downgrade(&self.shared), self.listeners.clone(), self.poll);
            thread::spawn(move || poll(shared, list, every));
        }
        receive
    }
}

#[cfg(test)]
mod tests;
