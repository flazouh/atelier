/// The rate the model hears at.
pub const SAMPLE_RATE: u32 = 16_000;

/// A recording shorter than this is a click of the button, not speech: it is not sent to the model.
pub const MIN_SECONDS: f32 = 0.3;
