use std::time::Duration;

/// How often the machine is sampled, and how often the providers are asked.
pub const LOAD_EVERY: Duration = Duration::from_secs(1);
pub const PROVIDERS_EVERY: Duration = Duration::from_secs(60);

/// How many samples of the processor the bar remembers: as many as it has bars for.
pub(super) const HISTORY: usize = 24;
