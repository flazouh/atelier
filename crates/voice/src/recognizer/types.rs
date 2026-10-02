/// The rate the model hears at.
pub const SAMPLE_RATE: u32 = 16_000;

/// A recording shorter than this is a click of the button, not speech: it is not sent to the model.
pub const MIN_SECONDS: f32 = 0.3;

/// Threads for the model: the most that helped on an M4 Pro (its 8 performance cores).
pub const THREADS: usize = 8;
