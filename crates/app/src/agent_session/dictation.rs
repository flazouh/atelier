//! Dictation in the session composer: the microphone, the speech model on this machine (`atelier_voice`), and what the box shows
//! meanwhile.
//!
//! One [`Engine`] serves the whole app, because the model takes about a gigabyte of memory. It lives in a global; the session whose
//! microphone was pressed is its owner until the press ends, and the engine's events go to that session.
//!
//! The first press fetches the model, so the composer shows the setup with real progress, then it listens. Later presses listen
//! at once. The sound cue plays when the microphone opens, as fluentai's timing contract asks, and when stop is pressed. A press
//! that ends without words says why for a few seconds and then clears.
use std::{cell::RefCell, rc::Rc, time::Duration};

use atelier_ui::{SetupPhase, VoiceMode};
use atelier_voice::{Cue, Engine, Event, START, STOP, files};
use futures_channel::mpsc::unbounded;
use futures_util::StreamExt;
use gpui_kit::{AnyWindowHandle, App, Context, Global, Task, WeakEntity, Window};

use super::AgentSession;

/// How long the reason for a failed press stays in the box.
const ERROR_SHOWN: Duration = Duration::from_secs(6);

/// The session whose press the engine is serving, and its window; none between presses.
type Owner = Rc<RefCell<Option<(WeakEntity<AgentSession>, AnyWindowHandle)>>>;

/// The app's one engine and the session it is serving.
struct Speech {
    engine: Rc<Engine>,
    owner: Owner,
}

impl Global for Speech {}

/// The engine, started on first use.
fn speech(cx: &mut App) -> (Rc<Engine>, Owner) {
    if let Some(speech) = cx.try_global::<Speech>() {
        return (speech.engine.clone(), speech.owner.clone());
    }
    let (tx, mut events) = unbounded::<Event>();
    let engine = Rc::new(Engine::spawn(move |event| {
        tx.unbounded_send(event).ok();
    }));
    let owner = Owner::default();
    let serving = owner.clone();
    App::spawn(cx, async move |cx| {
        while let Some(event) = events.next().await {
            let Some((session, window)) = serving.borrow().clone() else { continue };
            let ended = matches!(event, Event::Transcript(_) | Event::Failed(_));
            window
                .update(cx, |_, window, cx| {
                    session.update(cx, |session, cx| session.dictation_event(event, window, cx)).ok();
                })
                .ok();
            if ended {
                *serving.borrow_mut() = None;
            }
        }
    })
    .detach();
    cx.set_global(Speech { engine: engine.clone(), owner: owner.clone() });
    (engine, owner)
}

/// Loads the model in the background if it is on this machine, so the first press does not wait for it.
pub fn warm(cx: &mut App) {
    speech(cx).0.warm();
}

pub struct Dictation {
    start: Option<Cue>,
    stop: Option<Cue>,
    /// Clears a failed press's words after a while; dropping it cancels that.
    dismiss: Task<()>,
}

impl Dictation {
    pub fn new() -> Self {
        Self { start: Cue::new(START), stop: Cue::new(STOP), dismiss: Task::ready(()) }
    }
}

fn play(cue: &Option<Cue>) {
    if let Some(cue) = cue {
        cue.play();
    }
}

impl AgentSession {
    /// The user pressed the microphone.
    pub(super) fn dictation_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (engine, owner) = speech(cx);
        *owner.borrow_mut() = Some((cx.weak_entity(), window.window_handle()));
        self.dictation.dismiss = Task::ready(());
        if !engine.ready() {
            // The model has to come first; say so at once rather than when the worker gets to it.
            let fetched = files::dir().is_some_and(|dir| files::installed(&dir, &files::FILES));
            let total_mb = files::total_bytes(&files::FILES) as f32 / 1e6;
            self.composer.update(cx, |c, cx| {
                c.set_voice_total_mb(total_mb, cx);
                c.set_voice_setup(if fetched { SetupPhase::Prepare } else { SetupPhase::Download(0.) }, cx);
            });
        }
        engine.start();
    }

    /// The user pressed the stop square.
    pub(super) fn dictation_stop(&mut self, cx: &mut Context<Self>) {
        play(&self.dictation.stop);
        speech(cx).0.stop();
    }

    /// The engine said something about the press this session owns.
    fn dictation_event(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        let composer = self.composer.clone();
        match event {
            Event::Download { done, total } => composer.update(cx, |c, cx| {
                c.set_voice_total_mb(total as f32 / 1e6, cx);
                c.set_voice_setup(SetupPhase::Download(if total == 0 { 0. } else { done as f32 / total as f32 }), cx);
            }),
            Event::Prepare => composer.update(cx, |c, cx| c.set_voice_setup(SetupPhase::Prepare, cx)),
            Event::Ready => composer.update(cx, |c, cx| c.set_voice_setup(SetupPhase::Ready, cx)),
            Event::Listening => {
                play(&self.dictation.start);
                composer.update(cx, |c, cx| c.set_voice_listening(cx));
            }
            Event::Level(level) => composer.update(cx, |c, cx| c.set_voice_level(level, cx)),
            // The words come a moment after; the box is the user's again meanwhile.
            Event::Transcribing => composer.update(cx, |c, cx| c.set_voice_idle(cx)),
            Event::Transcript(words) => composer.update(cx, |c, cx| {
                c.set_voice_idle(cx);
                if !words.trim().is_empty() {
                    c.insert_transcript(words.trim(), window, cx);
                }
            }),
            Event::Failed(why) => {
                composer.update(cx, |c, cx| c.set_voice_error(why, cx));
                self.dictation.dismiss = cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(ERROR_SHOWN).await;
                    this.update(cx, |this, cx| {
                        this.composer.update(cx, |c, cx| {
                            if c.voice_mode() == VoiceMode::Failed {
                                c.set_voice_idle(cx);
                            }
                        })
                    })
                    .ok();
                });
            }
        }
    }
}
