use std::rc::Rc;

use atelier_voice::{Cue, Engine, START, STOP};
use gpui_kit::{Global, Task};

use super::types::Owner;

/// The app's one engine and the session it is serving.
pub(super) struct Speech {
    pub(super) engine: Rc<Engine>,
    pub(super) owner: Owner,
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
