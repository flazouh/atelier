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

/// Words kept on disk by an earlier run, on their way back to a session.
#[derive(Default)]
pub(super) struct Recovery {
    /// Presses the engine took up from disk, and the key of the session each was spoken in.
    pub(super) orphans: std::collections::HashMap<Press, String>,
    /// Their words, and that key, until a session is open to take them.
    pub(super) unclaimed: Vec<(String, String)>,
    /// The sessions open now, by key.
    pub(super) sessions: Vec<(gpui_kit::SharedString, WeakEntity<super::super::AgentSession>, AnyWindowHandle)>,
}

/// The app's one engine, the presses it is serving, where the model's setup is, and the person's choices.
pub(super) struct Speech {
    pub(super) engine: Rc<Engine>,
    pub(super) presses: Presses,
    /// The setup's last step, for a press that ends before the model is ready; none once it is.
    pub(super) phase: Rc<Cell<Option<SetupPhase>>>,
    pub(super) total_mb: Rc<Cell<f32>>,
    pub(super) prefs: Rc<RefCell<Prefs>>,
    pub(super) recovery: Rc<RefCell<Recovery>>,
}

impl Global for Speech {}

/// Where the dictation key's presses go: every composer and its window, the one last focused, and the one a press went to,
/// which keeps it until the press ends.
#[derive(Default)]
pub(super) struct KeyRoute {
    pub(super) composers: Vec<(WeakEntity<PromptInput>, AnyWindowHandle)>,
    pub(super) last: Option<WeakEntity<PromptInput>>,
    pub(super) target: Option<WeakEntity<PromptInput>>,
    /// Sends to the key's tracker.
    pub(super) inputs: Option<futures_channel::mpsc::UnboundedSender<(atelier_voice::hotkey::Input, std::time::Instant)>>,
    /// The system's own listener hears the key (macOS); elsewhere the shell's modifier events feed the tracker.
    pub(super) native: bool,
    /// Whether the key is down, as the shell's modifier events last said.
    pub(super) down: bool,
}

impl Global for KeyRoute {}

pub struct Dictation {
    pub(super) start: Option<Cue>,
    pub(super) stop: Option<Cue>,
    /// The press this session is recording, if any.
    pub(super) live: Option<Press>,
    /// This session's presses whose words wait for the model.
    pub(super) waiting: HashSet<Press>,
    /// The presses recorded for the reply box: their words go to it, not to the composer.
    pub(super) reply: HashSet<Press>,
    /// The reply box's press that still records, if any.
    pub(super) reply_live: Option<Press>,
    /// Clears a failed press's words after a while; dropping it cancels that.
    pub(super) dismiss: Task<()>,
}

impl Dictation {
    pub fn new() -> Self {
        Self { start: Cue::new(START), stop: Cue::new(STOP), live: None, waiting: HashSet::new(), reply: HashSet::new(), reply_live: None, dismiss: Task::ready(()) }
    }
}
