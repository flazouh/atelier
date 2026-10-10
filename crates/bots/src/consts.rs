//! The limits of the data.

/// The longest id of a bot, a playbook or a run, in characters.
pub const ID_MAX: usize = 32;
/// The longest name of a bot, in characters.
pub const NAME_MAX: usize = 40;
/// The longest one-line job of a bot, in characters.
pub const JOB_MAX: usize = 140;
/// The longest memory note, in characters.
pub const NOTE_MAX: usize = 2000;
/// The longest brief of a run, in characters.
pub const BRIEF_MAX: usize = 20_000;
/// What a step that asks first asks the person, when its turn comes.
pub const ASKS_FIRST_QUESTION: &str = "This step asks before it starts. May it start?";
