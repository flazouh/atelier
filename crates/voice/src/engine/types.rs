use std::time::Duration;

use crate::{Error, recognizer::Recognizer};

/// How often the level is read while it listens.
pub const LEVEL_EVERY: Duration = Duration::from_millis(33);

/// How often the download's progress is reported, at most.
pub const PROGRESS_EVERY: Duration = Duration::from_millis(50);

/// The longest a press records. A key whose release never came (the window lost focus, the app hung) stops here.
pub const MAX_PRESS: Duration = Duration::from_secs(120);

/// One press of the microphone, from [`Engine::start`](super::Engine::start). Every event about a press carries it, because
/// a press can end long after the next one began: its words wait for the model while the person goes on.
pub type Press = u64;

/// What the worker tells the app.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// `done` of `total` bytes of the model are on this machine.
    Download { done: u64, total: u64 },
    /// The model is on this machine and loads.
    Prepare,
    /// The model is in memory.
    Ready,
    /// Fetching or loading the model failed, for this reason. Recordings that wait for it are kept; the next press or
    /// [`Engine::fetch`](super::Engine::fetch) tries again.
    SetupFailed(String),
    /// The microphone is open.
    Listening(Press),
    /// How loud it is now, 0 to 1.
    Level(Press, f32),
    /// The press ended before the model was ready; its recording waits for it, kept on disk until its words are out.
    Waiting(Press),
    /// A recording kept from an earlier run, for the press `tag` named (see [`Engine::start`](super::Engine::start)). It waits
    /// for the model like any other, under this new press.
    Recovered(Press, String),
    /// The recording is being turned into words.
    Transcribing(Press),
    /// The words. Never empty: a press with no words ends in [`Event::Failed`] with the reason.
    Transcript(Press, String),
    /// The press ended without words, for this reason.
    Failed(Press, String),
    /// The press was taken back, and its recording thrown away.
    Cancelled(Press),
}

pub(super) enum Command {
    /// Load the model if it is on this machine.
    Warm,
    /// Fetch the model if it is missing, then load it.
    Fetch,
    /// Listen on this microphone, or the system's default; the tag says whose the press is.
    Start(Press, Option<String>, String),
    Stop(Press),
    Cancel(Press),
    /// From the setup thread: an event to pass on.
    Progress(Event),
    /// From the setup thread: the model, or why there is none.
    Loaded(Result<Box<Recognizer>, Error>),
}

/// Below this, a recording is not sound at all but silence the system hands over: a microphone that was refused, or a virtual
/// one with nothing routed to it. A quiet room reads about ten times louder.
pub const SILENT_BELOW: f32 = 1e-4;
