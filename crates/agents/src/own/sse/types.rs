/// The largest line the parser keeps. A line longer than this is a broken stream, not a big event.
pub(super) const MAX_LINE: usize = 16 * 1024 * 1024;
