use std::{str::FromStr, time::Duration};

/// Within this of the key going down, another key or a click takes the press back: it was a shortcut, not dictation. A
/// release within it is a tap, which keeps the microphone open hands-free.
pub const TAKE_BACK: Duration = Duration::from_millis(200);

/// Which key dictates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Key {
    /// Fn, or 🌐 on newer keyboards. macOS's own dictation uses it too, so "Press 🌐 key to" should be set to "Do nothing".
    #[default]
    Fn,
    RightOption,
    LeftOption,
}

impl Key {
    pub const ALL: [Key; 3] = [Key::Fn, Key::RightOption, Key::LeftOption];

    /// As the settings file names it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fn => "fn",
            Self::RightOption => "right-option",
            Self::LeftOption => "left-option",
        }
    }

    /// As a person reads it.
    pub fn words(self) -> &'static str {
        match self {
            Self::Fn => "Fn",
            Self::RightOption => "Right Option",
            Self::LeftOption => "Left Option",
        }
    }
}

impl FromStr for Key {
    type Err = ();

    fn from_str(name: &str) -> Result<Self, ()> {
        Self::ALL.into_iter().find(|k| k.name() == name).ok_or(())
    }
}

/// What happened to the key, or around it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Down,
    Up,
    /// Another key, or a click.
    Other,
    /// The window lost focus: a release may never come.
    Away,
}

/// What the microphone should do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Press,
    Release,
    Cancel,
}
