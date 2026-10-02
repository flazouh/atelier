use std::{str::FromStr, time::Duration};

/// Within this of the key going down, another key or a click takes the press back: it was a shortcut, not dictation. A
/// release within it is a tap, which keeps the microphone open hands-free.
pub const TAKE_BACK: Duration = Duration::from_millis(200);

/// Which key dictates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// Fn, or 🌐 on newer keyboards. macOS's own dictation uses it too, so "Press 🌐 key to" should be set to "Do nothing".
    Fn,
    RightOption,
    LeftOption,
    /// Either Alt. Off macOS the app hears only that a modifier is down, not which side, and not Fn at all; the right Alt is
    /// AltGr on many layouts and is not heard as Alt.
    Alt,
    /// Either Ctrl.
    Control,
}

impl Key {
    /// The keys this system can hear.
    #[cfg(target_os = "macos")]
    pub const ALL: &[Key] = &[Key::Fn, Key::RightOption, Key::LeftOption];
    #[cfg(not(target_os = "macos"))]
    pub const ALL: &[Key] = &[Key::Alt, Key::Control];

    /// The key a person who chose none dictates with. Off macOS there is none: every modifier there is in daily use (Alt-Tab,
    /// Alt to reach a menu), so the person picks one.
    #[cfg(target_os = "macos")]
    pub const DEFAULT: Option<Key> = Some(Key::Fn);
    #[cfg(not(target_os = "macos"))]
    pub const DEFAULT: Option<Key> = None;

    /// As the settings file names it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fn => "fn",
            Self::RightOption => "right-option",
            Self::LeftOption => "left-option",
            Self::Alt => "alt",
            Self::Control => "ctrl",
        }
    }

    /// As a person reads it.
    pub fn words(self) -> &'static str {
        match self {
            Self::Fn => "Fn",
            Self::RightOption => "Right Option",
            Self::LeftOption => "Left Option",
            Self::Alt => "Alt",
            Self::Control => "Ctrl",
        }
    }

    /// Whether this key is down, from which modifiers are: `None` for a key the modifiers cannot tell (Fn, one side of
    /// Option), which only the macOS listener hears.
    pub fn held(self, alt: bool, control: bool) -> Option<bool> {
        match self {
            Self::Alt => Some(alt),
            Self::Control => Some(control),
            Self::Fn | Self::RightOption | Self::LeftOption => None,
        }
    }
}

impl FromStr for Key {
    type Err = ();

    /// Only a key this system can hear: a settings file written on another system falls back to the default.
    fn from_str(name: &str) -> Result<Self, ()> {
        Self::ALL.iter().copied().find(|k| k.name() == name).ok_or(())
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
