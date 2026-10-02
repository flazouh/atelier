use std::time::Duration;

/// The time the story starts at, in seconds since the Unix epoch.
pub(super) const BASE: u64 = 1_790_700_000;

/// How long a live step lasts.
pub(super) const STEP: Duration = Duration::from_millis(1800);
