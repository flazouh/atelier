//! Dictation: the microphone, the speech model that runs on this machine, and the cues that mark its start and its stop.
//!
//! - [`capture`] listens on the default microphone and hands back 16 kHz mono audio and a live level.
//! - [`files`] knows the model's files (Phonon-2, exported to ONNX), where they live and how to fetch them with progress.
//! - [`recognizer`] runs the model on the CPU with ONNX Runtime. No Python, no server.
//! - [`engine`] is the one worker thread the app talks to: say start, say stop, hear events.
//! - [`demo`] is a made-up model and voice, for the gallery.
pub mod capture;
pub mod cue;
pub mod demo;
pub mod engine;
pub mod error;
pub mod files;
pub mod recognizer;

pub use cue::Cue;
pub use engine::{Engine, Event};
pub use error::Error;

/// The dictation cues from fluentai's tutor: a 0.18 s start and a 0.22 s stop.
pub const START: &[u8] = include_bytes!("../assets/dictation-start.wav");
pub const STOP: &[u8] = include_bytes!("../assets/dictation-stop.wav");
