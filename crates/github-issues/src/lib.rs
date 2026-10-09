//! GitHub Issues as a `tasks` provider. [`GithubIssues`] serves the issues of one repository through
//! `atelier_capabilities`, so the one tasks screen and the agent tools read and change them like any other tasks.
//!
//! - Every call goes through `gh api`, so the person's own `gh auth` sign-in is used and this crate holds no token.
//!   The `gh` runner is the [`Gh`] trait: tests give it a fake.
//! - A task's reference is `tasks:github:<owner>.<repo>:<number>`. An owner never holds a dot, so the first dot splits
//!   the account back into owner and repository.
//! - Pull requests are not tasks: the provider skips them.
//!
//! The mapping of open, closed and labels to the six statuses is in `docs/capabilities/tasks-v1.md`, section 4.2.
//! The label names that stand for a status or a priority are [`Options`].
//!
//! The crate has no UI. Every call blocks and may be slow, so none is made on the UI thread.
mod errors;
mod marker;
mod options;
mod provider;
mod runner;
mod scope;
mod time;
mod wire;

pub use options::{Options, PriorityLabels, StatusLabels};
pub use provider::GithubIssues;
pub use runner::{Call, Failure, Gh, GhCli, Method, Reply};
