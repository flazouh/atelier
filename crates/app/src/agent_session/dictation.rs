//! SPIKE: dictation in the session composer, run against a made-up model and a made-up voice (`atelier_voice::demo`), so the look
//! can be judged in the real app before the real speech model is wired in. It is off unless `ATELIER_DICTATION_SPIKE` is set.
//!
//! The first press finds no model, so the composer shows the setup and "downloads" it, then listens. Later presses listen at
//! once. Stop writes a made-up transcript into the box. The sound cue plays with the press, or when listening begins after the
//! setup, as fluentai's timing contract asks.
use std::time::{Duration, Instant};

use atelier_ui::SetupPhase;
use atelier_voice::{
    Cue, START, STOP,
    demo::{DOWNLOAD, PREPARE, READY, TRANSCRIPT, download_at, level},
};
use gpui_kit::{Context, Task, Window};

use super::AgentSession;

const FRAME: Duration = Duration::from_millis(33);

/// Whether the spike is on.
pub fn enabled() -> bool {
    std::env::var_os("ATELIER_DICTATION_SPIKE").is_some()
}

pub struct Dictation {
    installed: bool,
    start: Option<Cue>,
    stop: Option<Cue>,
    /// The setup, or the level feed, in flight; dropping it cancels it.
    task: Task<()>,
}

impl Dictation {
    pub fn new() -> Self {
        Self { installed: false, start: Cue::new(START), stop: Cue::new(STOP), task: Task::ready(()) }
    }
}

fn play(cue: &Option<Cue>) {
    if let Some(cue) = cue {
        cue.play();
    }
}

impl AgentSession {
    /// The user pressed the microphone.
    pub(super) fn dictation_start(&mut self, cx: &mut Context<Self>) {
        if self.dictation.installed {
            play(&self.dictation.start);
            return self.dictation_listen(cx);
        }
        // First use: the model has to come first.
        self.composer.update(cx, |c, cx| c.set_voice_setup(SetupPhase::Download(0.), cx));
        self.dictation.task = cx.spawn(async move |this, cx| {
            let begun = Instant::now();
            for (length, step) in [(DOWNLOAD, 0), (PREPARE, 1), (READY, 2)] {
                let at = begun.elapsed().as_secs_f32();
                loop {
                    cx.background_executor().timer(FRAME).await;
                    let t = ((begun.elapsed().as_secs_f32() - at) / length).min(1.);
                    let phase = match step {
                        0 => SetupPhase::Download(download_at(t)),
                        1 => SetupPhase::Prepare,
                        _ => SetupPhase::Ready,
                    };
                    if this.update(cx, |this, cx| this.composer.update(cx, |c, cx| c.set_voice_setup(phase, cx))).is_err() {
                        return;
                    }
                    if t >= 1. {
                        break;
                    }
                }
            }
            this.update(cx, |this, cx| {
                this.dictation.installed = true;
                play(&this.dictation.start);
                this.dictation_listen(cx);
            })
            .ok();
        });
    }

    /// Listening begins: the bars follow a made-up voice until the task is dropped.
    fn dictation_listen(&mut self, cx: &mut Context<Self>) {
        self.composer.update(cx, |c, cx| c.set_voice_listening(cx));
        self.dictation.task = cx.spawn(async move |this, cx| {
            let begun = Instant::now();
            loop {
                cx.background_executor().timer(FRAME).await;
                let level = level(begun.elapsed().as_secs_f32());
                if this.update(cx, |this, cx| this.composer.update(cx, |c, cx| c.set_voice_level(level, cx))).is_err() {
                    return;
                }
            }
        });
    }

    /// The user pressed the stop square.
    pub(super) fn dictation_stop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        play(&self.dictation.stop);
        self.dictation.task = Task::ready(());
        self.composer.update(cx, |c, cx| {
            c.set_voice_idle(cx);
            c.insert_transcript(TRANSCRIPT, window, cx);
        });
    }
}
