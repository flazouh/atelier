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

impl FromStr for Key {
    type Err = ();

    fn from_str(name: &str) -> Result<Self, ()> {
        match name {
            "fn" => Ok(Self::Fn),
            "right-option" => Ok(Self::RightOption),
            "left-option" => Ok(Self::LeftOption),
            _ => Err(()),
        }
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
