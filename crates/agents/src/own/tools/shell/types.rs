pub(super) const DEFAULT_TIMEOUT: u64 = 120;

pub(super) const MAX_TIMEOUT: u64 = 600;

/// The most output kept in memory. A command that prints more keeps the head; the rest is read and dropped
/// so the process never blocks on a full pipe.
pub(super) const KEEP: usize = 256 * 1024;
