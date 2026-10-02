use atelier_ui::{SetupPhase, VoiceMode};
use atelier_voice::{Event, files};
use gpui_kit::{Context, Task, Window};

use super::super::AgentSession;
use super::types::ERROR_SHOWN;
use super::helpers::{play, speech};

impl AgentSession {
    /// The user pressed the microphone.
    pub(in super::super) fn dictation_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
    pub(in super::super) fn dictation_stop(&mut self, cx: &mut Context<Self>) {
        play(&self.dictation.stop);
        speech(cx).0.stop();
    }

    /// The engine said something about the press this session owns.
    pub(super) fn dictation_event(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
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
