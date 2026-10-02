//! The data of a tracker, in plain types. Times are seconds since the Unix epoch. Nothing here knows a UI
//! type: the app maps a [`Task`] to what it shows.

mod structs;
mod types;

pub use structs::{Activity, NewTask, Patch, PrLink, Query, SessionLink, Task, TaskId};
pub use types::{ActivityKind, Assignee, Entry, Event, Priority, Status};
