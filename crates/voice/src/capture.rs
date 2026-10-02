//! The microphone: the default input device, heard as 16 kHz mono audio and a live level.
//!
//! The audio thread only copies what the device gives it into a buffer and adds up its energy. Turning that into 16 kHz
//! mono, and the energy into a level for the bars, happens off the audio thread, so the callback never waits.
//!
//! cpal needs ALSA headers to build on Linux, so Linux has no capture for now: [`Recorder::start`] says so.
#![cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]

mod helpers;
mod structs;
mod types;

pub use helpers::{level_from_rms, mono, to_16k};
pub use structs::Recorder;
pub use types::MAX_SECONDS;

#[cfg(test)]
use structs::Heard;

#[cfg(test)]
mod tests;
