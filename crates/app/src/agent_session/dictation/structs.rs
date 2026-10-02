use std::{cell::RefCell, rc::Rc};

use atelier_voice::{Cue, Engine, START, STOP};
use gpui_kit::{Global, Task};

use super::types::Owner;

/// What the person chose for dictation, kept in the settings file between runs.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Prefs {
    /// The microphone, by `atelier_voice::Device::id`; none: the system's default.
    pub(super) device: Option<String>,
    /// The microphone records only while it is held down.
    pub(super) hold: bool,
}

/// The app's one engine, the session it is serving, and the person's choices.
pub(super) struct Speech {
    pub(super) engine: Rc<Engine>,
    pub(super) owner: Owner,
    pub(super) prefs: Rc<RefCell<Prefs>>,
}

impl Global for Speech {}

pub struct Dictation {
    pub(super) start: Option<Cue>,
    pub(super) stop: Option<Cue>,
    /// Clears a failed press's words after a while; dropping it cancels that.
    pub(super) dismiss: Task<()>,
}

impl Dictation {
    pub fn new() -> Self {
        Self { start: Cue::new(START), stop: Cue::new(STOP), dismiss: Task::ready(()) }
    }
}
