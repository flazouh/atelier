//! The limits of the data.

/// The longest id of a bot or a playbook, in characters.
pub const ID_MAX: usize = 32;
/// The longest name of a bot, in characters.
pub const NAME_MAX: usize = 40;
/// The longest one-line job of a bot, in characters.
pub const JOB_MAX: usize = 140;
/// The longest memory note, in characters.
pub const NOTE_MAX: usize = 2000;
