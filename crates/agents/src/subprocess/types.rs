/// How many of the last lines of a process's stderr an end event carries.
pub(super) const STDERR_LINES: usize = 20;

/// The reason [`run`](super::run) gives for a command the reader cancelled.
pub const CANCELLED: &str = "cancelled";
