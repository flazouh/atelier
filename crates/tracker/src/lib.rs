//! The tasks behind a project. Three parts:
//!
//! - [`Tracker`]: the neutral interface. The app and the task views talk to it and to nothing else, so a
//!   Linear or a GitHub Issues backend can stand in for the local one (see `docs/tracker.md`).
//! - [`LocalTracker`]: one SQLite database per project, in the app's data folder.
//! - [`RuleSet`] and [`handle`]: the automation that moves a task along when a session or a pull request
//!   does something. Each rule is data; a team turns one off.
//!
//! The crate has no UI and no network. Every call blocks and may be slow, so none is made on the UI thread.
mod local;
mod model;
mod project;
mod rules;
mod tracker;

pub use local::LocalTracker;
pub use model::{
    Activity, ActivityKind, Assignee, Entry, Event, NewTask, Patch, PrLink, Priority, Query, SessionLink, Status, Task, TaskId,
};
pub use project::{ProjectKey, prefix_for};
pub use rules::{Decision, Handled, Rule, RuleSet, Signal, handle};
pub use tracker::{StopFlag, Subscription, Tracker, TrackerError, TrackerResult};
