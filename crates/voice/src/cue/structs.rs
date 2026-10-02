#[cfg(target_os = "macos")]
use objc2::{AnyThread, rc::Retained};
#[cfg(target_os = "macos")]
use objc2_app_kit::NSSound;
#[cfg(target_os = "macos")]
use objc2_foundation::NSData;

/// A sound cue that starts the moment it is asked to. Off macOS it is silent.
#[cfg(target_os = "macos")]
pub struct Cue(Retained<NSSound>);

#[cfg(target_os = "macos")]
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

/// A sound cue that starts the moment it is asked to. Off macOS it is silent.
#[cfg(not(target_os = "macos"))]
pub struct Cue;

#[cfg(not(target_os = "macos"))]
impl Cue {
    pub fn new(_: &'static [u8]) -> Option<Self> {
        Some(Self)
    }

    pub fn play(&self) {}
}
