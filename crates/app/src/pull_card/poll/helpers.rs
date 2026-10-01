use std::time::Duration;

use atelier_forge::ForgeError;

use super::types::{FIRST_FAILURE, MOST, RUNNING, SETTLED, Seen};

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
