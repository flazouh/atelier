//! The checks as atelier-ui draws them, with the Fault of a failing one. The forge lists a job's steps and its
//! log apart; the log is one text for the whole job. A step's part of it is found by the markers the runner
//! writes: each `Run` step opens with `##[group]Run ...`, each post step with `Post job cleanup.`, and the
//! job ends with `Cleaning up orphan processes`. Only failing steps keep their lines, so a job of thousands
//! of lines costs what its failure costs.

mod helpers;
mod structs;
mod types;

pub use helpers::{check_runs, split_log, state_of, steps, tolerated_or, wants_log};
pub use structs::JobLog;
pub use types::MAX_LOGS;
