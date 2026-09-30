//! How soon the card reads the pull request again: often while checks run, less once they settle, never
//! after a merge or a close. A failure backs off, and a rate limit waits as long as the forge says.
use std::time::Duration;
use lathe_forge::ForgeError;

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

/// The wait before the next read, or `None` to stop. `last_failure` is the wait after the failure
/// before this one, when the last read failed too.
pub fn next_delay(last_failure: Option<Duration>, seen: &Seen) -> Option<Duration> {
    let backed_off = || last_failure.map_or(FIRST_FAILURE, |d| d * 2).clamp(FIRST_FAILURE, MOST);
    match seen {
        Seen::Open { checks_running: true } => Some(RUNNING),
        Seen::Open { checks_running: false } => Some(SETTLED),
        Seen::Settled => None,
        Seen::Failed(ForgeError::RateLimited { retry_after: Some(seconds) }) => Some(Duration::from_secs(*seconds).max(FIRST_FAILURE)),
        // Waiting does not sign the reader in, install gh, or bring back what is gone.
        Seen::Failed(ForgeError::NotSignedIn | ForgeError::ToolMissing { .. } | ForgeError::NotFound(_) | ForgeError::Denied(_) | ForgeError::UnknownRemote(_)) => None,
        Seen::Failed(_) => Some(backed_off()),
    }
}

#[cfg(test)]
mod tests;
