//! The one worker thread dictation runs on. The app says [`Engine::start`], [`Engine::stop`] and [`Engine::cancel`] and hears
//! [`Event`]s; the listening and the recognizing happen here, and the fetching and loading on a thread beside it, all off the UI
//! thread.
//!
//! A press opens the microphone at once ([`Event::Listening`], then [`Event::Level`]), whether or not the model is here yet:
//! the person talks while it comes. The model is fetched if it is missing ([`Event::Download`]), checked and kept, then
//! loaded ([`Event::Prepare`], [`Event::Ready`]). [`Engine::fetch`] starts that early, when the person shows they mean to
//! dictate.
//!
//! Stop turns the recording into words ([`Event::Transcribing`], then [`Event::Transcript`]) at once if the model is ready.
//! If not, the recording waits ([`Event::Waiting`]) and is read the moment it is. Anything that fails ends the press with
//! [`Event::Failed`], in words for the person; a failed setup ([`Event::SetupFailed`]) keeps the recordings for the next try.

mod helpers;
mod structs;
mod types;

pub use structs::Engine;
pub use types::{Event, LEVEL_EVERY, MAX_PRESS, PROGRESS_EVERY, Press, SILENT_BELOW};

#[cfg(test)]
mod tests;
