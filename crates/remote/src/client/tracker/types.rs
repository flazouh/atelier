use std::time::Duration;

/// How often a listened-to tracker asks the host for changes made elsewhere.
pub const POLL: Duration = Duration::from_secs(3);
