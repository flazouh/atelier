/// How many times one request is tried in all.
pub(super) const TRIES: u32 = 4;

/// The longest atelier waits inside one call. A longer wait is the caller's to schedule.
pub(super) const LONGEST_WAIT: u64 = 60;

/// The most pages one list follows, so a forge that never ends cannot hold a call forever.
pub(super) const MOST_PAGES: usize = 500;
