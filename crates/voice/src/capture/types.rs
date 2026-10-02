/// The longest recording kept, in seconds. The model reads one clip at a time, and a clip this long is already five minutes of
/// talking; past it the oldest audio is not kept, rather than the buffer growing for ever.
pub const MAX_SECONDS: usize = 300;

/// One microphone the system offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// Stable between runs; hand it to [`Recorder::start`](super::Recorder::start).
    pub id: String,
    /// What the person reads: the name and how it connects, such as "USB Microphone (USB)".
    pub label: String,
    /// The one the system uses when none is chosen.
    pub is_default: bool,
}
