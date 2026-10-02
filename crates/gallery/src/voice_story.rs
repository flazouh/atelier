//! SPIKE, throwaway: the "Voice" story. A recording state for the prompt input, with amber waves that follow a made-up
//! voice and the tutor's dictation cues from fluentai. Start plays the cue before it changes state, as fluentai's
//! timing contract asks; Stop plays its cue as the waves settle.
use std::time::Instant;

use atelier_ui::{ActiveTheme, Button, ButtonSize, ButtonVariant, VoiceWaves};
use gpui_kit::{Context, IntoElement, ParentElement, Render, Styled, Window, div, px};

use crate::voice_sound::{Cue, START, STOP};

pub struct VoiceStory {
    recording: bool,
    since: Instant,
    start: Option<Cue>,
    stop: Option<Cue>,
}

impl VoiceStory {
    pub fn new() -> Self {
        Self { recording: false, since: Instant::now(), start: Cue::new(START), stop: Cue::new(STOP) }
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        let cue = if self.recording { &self.stop } else { &self.start };
        if let Some(cue) = cue {
            cue.play();
        }
        self.recording = !self.recording;
        self.since = Instant::now();
        cx.notify();
    }

    /// A made-up voice: words of a few syllables with a breath between them.
    fn level(&self) -> f32 {
        if !self.recording {
            return 0.;
        }
        let t = self.since.elapsed().as_secs_f32();
        let speaking = ((t * 1.15).sin() + 0.35 * (t * 0.37).sin()) > -0.25;
        let syllables = (t * 5.3).sin().abs() * (0.6 + 0.4 * (t * 1.7 + 1.).sin());
        let grit = 0.06 * (t * 31.).sin().abs();
        if speaking { (0.22 + 0.7 * syllables + grit).min(1.) } else { 0.04 }
    }
}

impl Render for VoiceStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let level = self.level();
        let toggle = cx.listener(|this, _, _, cx| this.toggle(cx));
        let panel = |height: f32, id: &'static str| {
            div().w_full().px(px(16.)).py(px(10.)).rounded(atelier_ui::theme::radius::card()).bg(theme.card).child(
                VoiceWaves::new(id).level(level).height(px(height)),
            )
        };
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .max_w(px(560.))
            .child(
                Button::new("voice-toggle")
                    .label(if self.recording { "Stop recording" } else { "Start recording" })
                    .variant(if self.recording { ButtonVariant::Secondary } else { ButtonVariant::Primary })
                    .size(ButtonSize::Md)
                    .on_click(toggle),
            )
            .child(panel(72., "voice-large"))
            .child(panel(32., "voice-composer"))
            .child(panel(20., "voice-compact"))
    }
}
