//! SPIKE: dictation sound cues. A sound cue that starts the moment it is asked to. On macOS it is an `NSSound` made from the clip's bytes
//! up front, so `play` only starts a sound that is already decoded. Elsewhere it is silent.

pub mod demo;

#[cfg(target_os = "macos")]
mod mac {
    use objc2::{AnyThread, rc::Retained};
    use objc2_app_kit::NSSound;
    use objc2_foundation::NSData;

    pub struct Cue(Retained<NSSound>);

    impl Cue {
        pub fn new(bytes: &'static [u8]) -> Option<Self> {
            let data = NSData::with_bytes(bytes);
            NSSound::initWithData(NSSound::alloc(), &data).map(Self)
        }

        /// Restarts from the beginning, so a quick second press still sounds.
        pub fn play(&self) {
            self.0.stop();
            self.0.play();
        }
    }
}

#[cfg(target_os = "macos")]
pub use mac::Cue;

#[cfg(not(target_os = "macos"))]
pub struct Cue;

#[cfg(not(target_os = "macos"))]
impl Cue {
    pub fn new(_: &'static [u8]) -> Option<Self> {
        Some(Self)
    }

    pub fn play(&self) {}
}

/// The tutor's dictation cues from fluentai: a 0.18 s start and a 0.22 s stop.
pub const START: &[u8] = include_bytes!("../assets/dictation-start.wav");
pub const STOP: &[u8] = include_bytes!("../assets/dictation-stop.wav");
