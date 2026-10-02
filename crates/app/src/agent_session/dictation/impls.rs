use atelier_ui::{SetupPhase, VoiceMode};
use atelier_voice::{Event, access::{self, Access}};
use gpui_kit::{Context, SharedString, Task, Window};

use super::super::AgentSession;
use super::types::ERROR_SHOWN;
use super::helpers::{DEFAULT_ID, choose, device_rows, fetch, play, prefs, setup_now, speech};

impl AgentSession {
    /// The microphone was pressed, and the box already shows it listening: the cue plays now and the engine opens the
    /// microphone, while the model comes alongside if it is not here yet.
    pub(in super::super) fn dictation_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dictation.dismiss = Task::ready(());
        match access::status() {
            Access::Granted => play(&self.dictation.start),
            // The engine asks; this press cannot record, and says so at once rather than pretend to listen.
            Access::Unasked => self.composer.update(cx, |c, cx| c.set_voice_error("Allow microphone access, then press again.", cx)),
            Access::Refused => {
                let why = atelier_voice::Error::Access.to_string();
                return self.dictation_failed(why, cx);
            }
        }
        let (engine, presses) = speech(cx);
        let press = engine.start(prefs(cx).device, self.key.to_string());
        presses.borrow_mut().insert(press, (cx.weak_entity(), window.window_handle()));
        self.dictation.live = Some(press);
    }

    /// The press ended: the stop cue plays, and the words come when the engine has them.
    pub(in super::super) fn dictation_stop(&mut self, cx: &mut Context<Self>) {
        let Some(press) = self.dictation.live.take() else { return };
        play(&self.dictation.stop);
        speech(cx).0.stop(press);
    }

    /// The press was taken back: no cue, no words.
    pub(in super::super) fn dictation_cancel(&mut self, cx: &mut Context<Self>) {
        let Some(press) = self.dictation.live.take() else { return };
        speech(cx).0.cancel(press);
    }

    /// The person threw away the words waiting for the model.
    pub(in super::super) fn dictation_discard(&mut self, cx: &mut Context<Self>) {
        let engine = speech(cx).0;
        for press in self.dictation.waiting.drain() {
            engine.cancel(press);
        }
    }

    /// The microphone menu is about to show: bring its rows and its choices up to date, and start fetching the model, since
    /// the person means to dictate.
    pub(in super::super) fn dictation_devices(&mut self, cx: &mut Context<Self>) {
        fetch(cx);
        let chosen = prefs(cx);
        let (rows, selected) = device_rows(&atelier_voice::devices(), chosen.device.as_deref());
        let hold = chosen.hold;
        self.composer.update(cx, |c, cx| {
            c.set_voice_devices(rows, Some(selected.into()), cx);
            c.set_voice_hold(hold, cx);
        });
    }

    /// The user chose a microphone, or the default row.
    pub(in super::super) fn dictation_device(&mut self, id: Option<SharedString>, cx: &mut Context<Self>) {
        choose(cx, |prefs| prefs.device = id.filter(|id| id != DEFAULT_ID).map(|id| id.to_string()));
    }

    /// The user turned "Hold to record" on or off.
    pub(in super::super) fn dictation_hold(&mut self, hold: bool, cx: &mut Context<Self>) {
        choose(cx, |prefs| prefs.hold = hold);
    }

    /// The model's setup moved on (`step`), or failed (`None`). Shown only while this session has words waiting for it, and
    /// never over a press that is recording.
    pub(super) fn dictation_setup(&mut self, step: Option<SetupPhase>, event: Event, _: &mut Window, cx: &mut Context<Self>) {
        if self.dictation.waiting.is_empty() || self.composer.read(cx).voice_mode() == VoiceMode::Listening {
            return;
        }
        match (step, event) {
            (Some(phase), _) => {
                let (_, total_mb) = setup_now(cx);
                self.composer.update(cx, |c, cx| {
                    c.set_voice_total_mb(total_mb, cx);
                    c.set_voice_setup(phase, cx);
                });
            }
            (None, _) => self.dictation_failed("The speech model did not download. Your words are kept: press the microphone to try again.".into(), cx),
        }
    }

    /// The engine said something about one of this session's presses.
    pub(super) fn dictation_event(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        let listening = self.composer.read(cx).voice_mode() == VoiceMode::Listening;
        match event {
            Event::Level(press, level) if self.dictation.live == Some(press) => self.composer.update(cx, |c, cx| c.set_voice_level(level, cx)),
            Event::Waiting(press) => {
                self.dictation.waiting.insert(press);
                let (phase, total_mb) = setup_now(cx);
                if !listening && let Some(phase) = phase {
                    self.composer.update(cx, |c, cx| {
                        c.set_voice_total_mb(total_mb, cx);
                        c.set_voice_setup(phase, cx);
                    });
                }
            }
            Event::Transcript(press, words) => {
                self.press_over(press, listening, cx);
                self.composer.update(cx, |c, cx| c.insert_transcript(words.trim(), window, cx));
            }
            Event::Failed(press, why) => {
                self.press_over(press, true, cx);
                if !listening {
                    self.dictation_failed(why, cx);
                }
            }
            Event::Cancelled(press) => self.press_over(press, listening, cx),
            _ => {}
        }
    }

    /// `press` is finished. When it was the last whose words waited, the box goes back to the user, unless it records again.
    fn press_over(&mut self, press: atelier_voice::Press, listening: bool, cx: &mut Context<Self>) {
        if self.dictation.live == Some(press) {
            self.dictation.live = None;
            // The engine ended a press the box still shows as listening: the microphone could not open.
            self.composer.update(cx, |c, cx| c.release_mic(cx));
        }
        let was_waiting = self.dictation.waiting.remove(&press);
        if was_waiting && self.dictation.waiting.is_empty() && !listening {
            self.composer.update(cx, |c, cx| c.set_voice_idle(cx));
        }
    }

    /// Says why, for a few seconds, unless the user has moved on.
    fn dictation_failed(&mut self, why: String, cx: &mut Context<Self>) {
        self.composer.update(cx, |c, cx| c.set_voice_error(why, cx));
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
