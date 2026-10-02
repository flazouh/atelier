//! Checks: what ran against a commit. A check on a pull request and a job on its run are one thing seen
//! from two pages (`docs/glossary.md`), so one type holds both.

mod structs;
mod types;

pub use structs::{Check, Job, JobRef, RunInfo, Step};
pub use types::{CheckStatus, Conclusion};
