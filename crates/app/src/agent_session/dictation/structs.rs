use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
};

use atelier_ui::{PromptInput, SetupPhase};
use atelier_voice::{Cue, Engine, Press, START, STOP};
use gpui_kit::{AnyWindowHandle, Global, Task, WeakEntity};

use super::types::Presses;

/// What the person chose for dictation, kept in the settings file between runs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prefs {
    /// The microphone, by `atelier_voice::Device::id`; none: the system's default.
    pub device: Option<String>,
    /// The microphone button records only while it is held down.
    pub hold: bool,
    /// The key that dictates; none: no key.
    pub key: Option<atelier_voice::hotkey::Key>,
}

/// The app's one engine, the presses it is serving, where the model's setup is, and the person's choices.
pub(super) struct Speech {
    pub(super) engine: Rc<Engine>,
    pub(super) presses: Presses,
    /// The setup's last step, for a press that ends before the model is ready; none once it is.
    pub(super) phase: Rc<Cell<Option<SetupPhase>>>,
    pub(super) total_mb: Rc<Cell<f32>>,
    pub(super) prefs: Rc<RefCell<Prefs>>,
}

impl Global for Speech {}

/// Where the dictation key's presses go: every composer and its window, the one last focused, and the one a press went to,
/// which keeps it until the press ends.
#[derive(Default)]
pub(super) struct KeyRoute {
    pub(super) composers: Vec<(WeakEntity<PromptInput>, AnyWindowHandle)>,
    pub(super) last: Option<WeakEntity<PromptInput>>,
    pub(super) target: Option<WeakEntity<PromptInput>>,
    /// Sends `Input::Away` to the key's tracker; none where there is no key.
    pub(super) away: Option<futures_channel::mpsc::UnboundedSender<(atelier_voice::hotkey::Input, std::time::Instant)>>,
}

impl Global for KeyRoute {}

pub struct Dictation {
    pub(super) start: Option<Cue>,
    pub(super) stop: Option<Cue>,
    /// The press this session is recording, if any.
    pub(super) live: Option<Press>,
    /// This session's presses whose words wait for the model.
    pub(super) waiting: HashSet<Press>,
    /// Clears a failed press's words after a while; dropping it cancels that.
    pub(super) dismiss: Task<()>,
}

impl Dictation {
    pub fn new() -> Self {
        Self { start: Cue::new(START), stop: Cue::new(STOP), live: None, waiting: HashSet::new(), dismiss: Task::ready(()) }
    }
}
