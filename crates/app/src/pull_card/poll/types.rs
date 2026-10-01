use std::time::Duration;

use atelier_forge::ForgeError;

/// While checks run.
pub const RUNNING: Duration = Duration::from_secs(10);

/// An open pull request with nothing running.
pub const SETTLED: Duration = Duration::from_secs(30);

/// The first wait after a failure, and the least after a rate limit.
pub const FIRST_FAILURE: Duration = Duration::from_secs(10);

/// The longest wait.
pub const MOST: Duration = Duration::from_secs(300);

/// What the last read found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Seen {
    Open { checks_running: bool },
    /// Merged or closed: nothing will change on its own.
    Settled,
    Failed(ForgeError),
}
