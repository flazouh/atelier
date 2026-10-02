//! SPIKE, throwaway: the "Voice" story. The whole dictation flow from the design system, run against a made-up model and
//! a made-up voice, with the tutor's dictation cues from fluentai. It runs twice: in the stand-alone `VoiceInput` bar and
//! in a real `PromptInput` composer, whose microphone sits before Send. Stop in the composer writes a made-up transcript
//! in the box.
//!
//! The first press finds no speech model, so the bar morphs into the setup bar and "downloads" it (a few seconds, with
//! a stall in the middle as real downloads have), then "loads" it, then listens. Later presses listen at once. "Forget
//! the model" brings the first use back. The cue plays with the press, or when listening begins after a setup, as
//! fluentai's timing contract asks.

mod structs;
mod types;

pub use structs::VoiceStory;
