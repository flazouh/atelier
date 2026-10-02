//! Dictation: the microphone, the speech model that runs on this machine, and the cues that mark its start and its stop.
//!
//! - [`access`] asks the system whether atelier may use the microphone, and says so when it may not.
//! - [`capture`] listens on the default microphone and hands back 16 kHz mono audio and a live level.
//! - [`files`] knows the model's files (Parakeet TDT v2, exported to ONNX), where they live and how to fetch them with progress.
//! - [`recognizer`] runs the model on the CPU with ONNX Runtime. No Python, no server.
//! - [`engine`] is the one worker thread the app talks to: say start, say stop, hear events.
//! - [`demo`] is a made-up model and voice, for the gallery.
pub mod access;
pub mod capture;
pub mod cue;
pub mod demo;
pub mod engine;
pub mod error;
pub mod files;
pub mod recognizer;
mod types;

pub use cue::Cue;
pub use capture::{Device, devices};
pub use engine::{Engine, Event};
pub use error::Error;
pub use types::{START, STOP};
