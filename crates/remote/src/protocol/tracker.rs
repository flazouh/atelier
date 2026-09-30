//! The tracker's calls over the pipe (protocol 5): the `Tracker` trait, call by call, as serde values.

use lathe_tracker::{Activity, Entry, NewTask, Patch, Query, Task, TaskId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TrackerCall {
    /// Opens the store on the host, and answers its name.
    Name,
    List { query: Query },
    Get { id: TaskId },
    Create { new: NewTask, by: String },
    CreateMany { new: Vec<NewTask>, by: String },
    Update { id: TaskId, patch: Patch, by: String },
    Record { id: TaskId, entry: Entry, by: String },
    Activity { id: TaskId },
    TasksOfSession { session_id: String },
    TasksOfPr { number: u64 },
    Labels,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TrackerReply {
    Name(String),
    Tasks(Vec<Task>),
    Found(Option<Task>),
    Task(Task),
    Activity(Activity),
    Log(Vec<Activity>),
    Ids(Vec<TaskId>),
    Labels(Vec<String>),
}
