//! The interface to a tracker. Every call blocks and may be slow, so none is made on the UI thread.
use std::sync::mpsc::Receiver;

use crate::{Activity, Entry, Event, NewTask, Patch, Query, Task, TaskId};

#[derive(Clone, Debug, PartialEq, Eq)]
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
    fn subscribe(&self) -> Receiver<Event>;
}
