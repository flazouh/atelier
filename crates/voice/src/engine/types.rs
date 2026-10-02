use std::{
    time::{Duration},
};

/// How often the level is read while it listens.
pub const LEVEL_EVERY: Duration = Duration::from_millis(33);

/// How often the download's progress is reported, at most.
pub const PROGRESS_EVERY: Duration = Duration::from_millis(50);

/// How long "ready" stays up before the microphone opens, after a setup.
pub const READY_BEAT: Duration = Duration::from_millis(600);

/// What the worker tells the app.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// `done` of `total` bytes of the model are on this machine.
    Download { done: u64, total: u64 },
    /// The model is on this machine and loads.
    Prepare,
    /// The setup is finished; the microphone opens in a moment.
    Ready,
    /// The microphone is open.
    Listening,
    /// How loud it is now, 0 to 1.
    Level(f32),
    /// The recording is being turned into words.
    Transcribing,
    /// The words. Empty when nothing was said.
    Transcript(String),
    /// The press ended without words, for this reason.
    Failed(String),
}

pub(super) enum Command {
    Warm,
    Start,
    Stop,
}
