//! The sound cues that mark dictation: a short chime when it starts listening and another when it stops. A cue starts the
//! moment it is asked to: on macOS it is an `NSSound` made from the clip's bytes up front, so `play` only starts a sound that is
//! already decoded. Elsewhere it is silent.

mod structs;

pub use structs::Cue;
