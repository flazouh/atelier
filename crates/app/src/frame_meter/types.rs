use std::time::Duration;

/// Frames a report covers.
pub(super) const FRAMES: usize = 300;

/// A frame must fit this: 120 Hz.
pub(super) const LIMIT: Duration = Duration::from_micros(8_333);

/// What `ATELIER_FRAMES` asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Off,
    /// A report every 300 frames.
    Reports,
    /// The reports, and a line per frame.
    Each,
}
