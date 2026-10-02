//! Dictation in the session composer: the microphone, the speech model on this machine (`atelier_voice`), and what the box shows
//! meanwhile.
//!
//! One `Engine` serves the whole app, because the model takes about a gigabyte of memory. It lives in a global with every press
//! it has not finished, and whose each is, so words that come late still land in the session that asked for them.
//!
//! A press listens at once, with its cue, even the very first: the model downloads and loads alongside, and starts as soon as the
//! microphone menu opens. Words that end before the model is ready wait for it, and only then does the box show the setup.
//! The dictation key (Fn unless the settings file says otherwise) presses the focused composer's microphone: held, it talks;
//! tapped, it stays open until the next tap.

mod helpers;
mod impls;
mod structs;
mod types;

pub use helpers::{hear_key, key_away, start_up, warm};
pub use structs::Dictation;
