//! Dictation in the session composer: the microphone, the speech model on this machine (`atelier_voice`), and what the box shows
//! meanwhile.
//!
//! One `Engine` serves the whole app, because the model takes about a gigabyte of memory. It lives in a global; the session whose
//! microphone was pressed is its owner until the press ends, and the engine's events go to that session.
//!
//! The first press fetches the model, so the composer shows the setup with real progress, then it listens. Later presses listen
//! at once. The sound cue plays when the microphone opens, as fluentai's timing contract asks, and when stop is pressed. A press
//! that ends without words says why for a few seconds and then clears.

mod helpers;
mod impls;
mod structs;
mod types;

pub use helpers::warm;
pub use structs::Dictation;
