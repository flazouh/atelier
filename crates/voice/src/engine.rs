//! The one worker thread dictation runs on. The app says [`Engine::start`] and [`Engine::stop`] and hears [`Event`]s; the
//! fetching, the loading, the listening and the recognizing all happen here, off the UI thread.
//!
//! A press of the microphone goes through up to four steps, and the app is told of each:
//!
//! 1. The model is not on this machine: it is fetched ([`Event::Download`]), checked, and kept.
//! 2. The model is not in memory: it loads ([`Event::Prepare`], a few seconds).
//! 3. If either of those ran, [`Event::Ready`], and a beat for the person to see it.
//! 4. The microphone opens ([`Event::Listening`]) and its level streams in ([`Event::Level`]) until stop.
//!
//! Stop turns the recording into words ([`Event::Transcribing`], then [`Event::Transcript`]). Anything that fails ends the
//! press with [`Event::Failed`], in words for the person.

mod structs;
mod types;

pub use structs::Engine;
pub use types::{Event, LEVEL_EVERY, PROGRESS_EVERY, READY_BEAT};
