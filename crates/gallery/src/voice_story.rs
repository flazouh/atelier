//! SPIKE, throwaway: the "Voice" story. The whole dictation bar from the design system, run against a made-up model and a
//! made-up voice, with the tutor's dictation cues from fluentai.
//!
//! The first press finds no speech model, so the bar morphs into the setup bar and "downloads" it (a few seconds, with
//! a stall in the middle as real downloads have), then "loads" it, then listens. Later presses listen at once. "Forget
//! the model" brings the first use back. The cue plays with the press, or when listening begins after a setup, as
//! fluentai's timing contract asks.
use std::time::{Duration, Instant};

use atelier_ui::{
    ActiveTheme, Button, ButtonSize, ButtonVariant, SetupPhase, VoiceInput, VoiceInputEvent, VoiceMode, VoiceWaves,
};
use gpui_kit::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Task, Window, div, px};

use crate::voice_sound::{Cue, START, STOP};

/// How long the made-up download, load and ready beat take.
const DOWNLOAD: f32 = 4.4;
const PREPARE: f32 = 1.8;
const READY: f32 = 0.7;
const FRAME: Duration = Duration::from_millis(33);

pub struct VoiceStory {
    voice: Entity<VoiceInput>,
    installed: bool,
    since: Instant,
    start: Option<Cue>,
    stop: Option<Cue>,
    /// The setup in flight; dropping it cancels it.
    setup: Option<Task<()>>,
    _ticker: Task<()>,
    _events: gpui_kit::Subscription,
}

/// The made-up download: quick, a stall at about 60%, quick again. `t` is 0 to 1 through it.
fn download_at(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    if t < 0.55 {
        (t / 0.55).powf(0.9) * 0.6
    } else if t < 0.72 {
        0.6 + (t - 0.55) * 0.05
    } else {
        0.6085 + (t - 0.72) / 0.28 * (1. - 0.6085)
    }
}

impl VoiceStory {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let voice = cx.new(|_| VoiceInput::new());
        let events = cx.subscribe(&voice, |this, _, event: &VoiceInputEvent, cx| match event {
            VoiceInputEvent::Start => this.start(cx),
            VoiceInputEvent::Stop => this.stop(cx),
        });
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(FRAME).await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    return;
                }
            }
        });
        Self { voice, installed: false, since: Instant::now(), start: Cue::new(START), stop: Cue::new(STOP), setup: None, _ticker: ticker, _events: events }
    }

    fn cue(cue: &Option<Cue>) {
        if let Some(cue) = cue {
            cue.play();
        }
    }

    fn listen(&mut self, cx: &mut Context<Self>) {
        self.since = Instant::now();
        self.voice.update(cx, |v, cx| v.set_listening(cx));
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        if self.installed {
            Self::cue(&self.start);
            self.listen(cx);
            return;
        }
        // First use: the model has to come first.
        self.voice.update(cx, |v, cx| v.set_setup(SetupPhase::Download(0.), cx));
        self.setup = Some(cx.spawn(async move |this, cx| {
            let begun = Instant::now();
            let steps = [(DOWNLOAD, 0), (PREPARE, 1), (READY, 2)];
            for (length, step) in steps {
                let at = begun.elapsed().as_secs_f32();
                loop {
                    cx.background_executor().timer(FRAME).await;
                    let t = ((begun.elapsed().as_secs_f32() - at) / length).min(1.);
                    let phase = match step {
                        0 => SetupPhase::Download(download_at(t)),
                        1 => SetupPhase::Prepare,
                        _ => SetupPhase::Ready,
                    };
                    if this.update(cx, |this, cx| this.voice.update(cx, |v, cx| v.set_setup(phase, cx))).is_err() {
                        return;
                    }
                    if t >= 1. {
                        break;
                    }
                }
            }
            this.update(cx, |this, cx| {
                this.installed = true;
                this.setup = None;
                Self::cue(&this.start);
                this.listen(cx);
            })
            .ok();
        }));
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        Self::cue(&self.stop);
        self.voice.update(cx, |v, cx| v.set_idle(cx));
    }

    fn forget(&mut self, cx: &mut Context<Self>) {
        self.setup = None;
        self.installed = false;
        self.voice.update(cx, |v, cx| v.set_idle(cx));
        cx.notify();
    }

    /// Feeds the bars a made-up voice while listening.
    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.voice.read(cx).mode() == VoiceMode::Listening {
            let level = self.level();
            self.voice.update(cx, |v, cx| v.set_level(level, cx));
            cx.notify();
        }
    }

    /// A made-up voice: words of a few syllables with a breath between them.
    fn level(&self) -> f32 {
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
        let listening = self.voice.read(cx).mode() == VoiceMode::Listening;
        let level = if listening { self.level() } else { 0. };
        let forget = cx.listener(|this, _, _, cx| this.forget(cx));
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .max_w(px(560.))
            .child(self.voice.clone())
            .child(
                div().flex().child(
                    Button::new("voice-forget").label("Forget the model (first use again)").variant(ButtonVariant::Secondary).size(ButtonSize::Sm).on_click(forget),
                ),
            )
            .child(
                div().w_full().px(px(16.)).py(px(10.)).rounded(atelier_ui::theme::radius::card()).bg(theme.card).child(
                    VoiceWaves::new("voice-large").level(level).height(px(72.)).bars(56),
                ),
            )
    }
}
