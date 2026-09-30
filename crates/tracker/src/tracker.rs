//! The interface to a tracker. Every call blocks and may be slow, so none is made on the UI thread.
use std::{
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Receiver,
    },
};

use serde::{Deserialize, Serialize};

use crate::{Activity, Entry, Event, NewTask, Patch, Query, Task, TaskId};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackerError {
    /// No task has this id.
    NotFound(TaskId),
    /// The request cannot be made sense of: an empty title, a task that is its own parent.
    Invalid(String),
    /// The store failed: the database is locked, the disk is full, a newer app wrote it.
    Storage(String),
    /// This tracker cannot do that. A backend says so for what its service has no place for.
    Unsupported(String),
}

impl std::fmt::Display for TrackerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "no task {id}"),
            Self::Invalid(why) => write!(f, "{why}"),
            Self::Storage(why) => write!(f, "the task store failed: {why}"),
            Self::Unsupported(what) => write!(f, "this tracker cannot {what}"),
        }
    }
}

impl std::error::Error for TrackerError {}

pub type TrackerResult<T> = Result<T, TrackerError>;

/// The changes a [`Tracker`] tells, until this is dropped. It reads as the receiver it holds
/// (`recv`, `recv_timeout`, `try_recv`). Dropping it tells the tracker to stop: a backend that polls a
/// service stops the poll at its next tick, and reads nothing after it.
pub struct Subscription {
    events: Receiver<Event>,
    stop: Arc<AtomicBool>,
}

/// What the tracker keeps of a [`Subscription`]: it says whether the reader is gone.
#[derive(Clone, Debug)]
pub struct StopFlag(Arc<AtomicBool>);

impl StopFlag {
    /// The subscription was dropped: send nothing more and read nothing more for it.
    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl Subscription {
    /// The reader's end, and the flag for the backend that sends to `events`.
    pub fn new(events: Receiver<Event>) -> (Self, StopFlag) {
        let stop = Arc::new(AtomicBool::new(false));
        (Self { events, stop: stop.clone() }, StopFlag(stop))
    }
}

impl Deref for Subscription {
    type Target = Receiver<Event>;
    fn deref(&self) -> &Receiver<Event> {
        &self.events
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

/// The tasks of one project. `by` is who acts, for the activity log: a person's name, an agent's name, or
/// `rule:<id>` for the automation.
pub trait Tracker: Send + Sync {
    /// Which tracker this is: "local", "linear", "github".
    fn name(&self) -> &str;

    /// The tasks that match, most recently changed first. An empty query is every task.
    fn list(&self, query: &Query) -> TrackerResult<Vec<Task>>;

    fn get(&self, id: &TaskId) -> TrackerResult<Option<Task>>;

    /// Makes a task with the next short id ("LAT-42") and logs that it was created.
    fn create(&self, new: &NewTask, by: &str) -> TrackerResult<Task>;

    /// Many tasks in one step, for an import. Logs each as created.
    fn create_many(&self, new: &[NewTask], by: &str) -> TrackerResult<Vec<Task>>;

    /// Changes fields, and logs what changed: the status with its old and new value, the assignee, the
    /// other fields as an edit. A patch that changes nothing writes nothing and logs nothing.
    fn update(&self, id: &TaskId, patch: &Patch, by: &str) -> TrackerResult<Task>;

    /// Appends to the activity log. A session or a pull request also becomes a link of the task.
    fn record(&self, id: &TaskId, entry: &Entry, by: &str) -> TrackerResult<Activity>;

    /// The activity of a task, oldest first.
    fn activity(&self, id: &TaskId) -> TrackerResult<Vec<Activity>>;

    /// The tasks a session is linked to.
    fn tasks_of_session(&self, session_id: &str) -> TrackerResult<Vec<TaskId>>;

    /// The tasks a pull request (by number, in the project's repository) is linked to.
    fn tasks_of_pr(&self, number: u64) -> TrackerResult<Vec<TaskId>>;

    /// The labels in use, sorted.
    fn labels(&self) -> TrackerResult<Vec<String>>;

    /// Changes made through this tracker from now on. Each call gives its own receiver; drop it to stop.
    /// A change made by another process, or on the service a backend syncs with, arrives here too when the
    /// backend can tell.
    fn subscribe(&self) -> Subscription;
}
