use std::time::Duration;

/// How often an open card with its Live part reads again.
pub(super) const LIVE_EVERY: Duration = Duration::from_secs(10);
