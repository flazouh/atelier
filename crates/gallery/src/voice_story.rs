//! SPIKE, throwaway: the "Voice" story. The whole dictation flow from the design system, run against a made-up model and
//! a made-up voice, with the tutor's dictation cues from fluentai. It runs twice: in the stand-alone `VoiceInput` bar and
//! in a real `PromptInput` composer, whose microphone sits before Send. Stop in the composer writes a made-up transcript
//! in the box.
//!
//! The first press finds no speech model, so the bar morphs into the setup bar and "downloads" it (a few seconds, with
//! a stall in the middle as real downloads have), then "loads" it, then listens. Later presses listen at once. "Forget
//! the model" brings the first use back. The cue plays with the press, or when listening begins after a setup, as
//! fluentai's timing contract asks.
use std::time::{Duration, Instant};

use atelier_ui::{
    ActiveTheme, Button, ButtonSize, ButtonVariant, PromptInput, PromptInputEvent, SetupPhase, VoiceInput, VoiceInputEvent, VoiceMode,
    VoiceWaves,
};
use gpui_kit::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Task, Window, div, px};

use atelier_voice::{
    Cue, START, STOP,
    demo::{DOWNLOAD, PREPARE, READY, TRANSCRIPT, download_at, level},
};

const FRAME: Duration = Duration::from_millis(33);

/// Which face the made-up session drives.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Face {
    Bar,
    Composer,
}

pub struct VoiceStory {
    voice: Entity<VoiceInput>,
    composer: Entity<PromptInput>,
    face: Face,
    installed: bool,
    since: Instant,
    start: Option<Cue>,
    stop: Option<Cue>,
    /// The setup in flight; dropping it cancels it.
    setup: Option<Task<()>>,
    _ticker: Task<()>,
    _events: [gpui_kit::Subscription; 2],
}

impl VoiceStory {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let voice = cx.new(|_| VoiceInput::new());
        let composer = cx.new(|cx| {
            let mut input = PromptInput::new("Ask Claude Code", "", window, cx);
            input.set_dictation(true, cx);
            input
        });
        let bar_events = cx.subscribe_in(&voice, window, |this, _, event: &VoiceInputEvent, window, cx| {
            this.face = Face::Bar;
            match event {
                VoiceInputEvent::Start => this.start(cx),
                VoiceInputEvent::Stop => this.stop(window, cx),
            }
        });
        let composer_events = cx.subscribe_in(&composer, window, |this, _, event: &PromptInputEvent, window, cx| {
            this.face = Face::Composer;
            match event {
                PromptInputEvent::DictationStart => this.start(cx),
                PromptInputEvent::DictationStop => this.stop(window, cx),
                _ => {}
            }
        });
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(FRAME).await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    return;
                }
            }
        });
        Self {
            voice,
            composer,
            face: Face::Bar,
            installed: false,
            since: Instant::now(),
            start: Cue::new(START),
            stop: Cue::new(STOP),
            setup: None,
            _ticker: ticker,
            _events: [bar_events, composer_events],
        }
    }

    fn cue(cue: &Option<Cue>) {
        if let Some(cue) = cue {
            cue.play();
        }
    }

    fn mode(&self, cx: &Context<Self>) -> VoiceMode {
        match self.face {
            Face::Bar => self.voice.read(cx).mode(),
            Face::Composer => self.composer.read(cx).voice_mode(),
        }
    }

    fn show_setup(&mut self, phase: SetupPhase, cx: &mut Context<Self>) {
        match self.face {
            Face::Bar => self.voice.update(cx, |v, cx| v.set_setup(phase, cx)),
            Face::Composer => self.composer.update(cx, |c, cx| c.set_voice_setup(phase, cx)),
        }
    }

    fn show_idle(&mut self, cx: &mut Context<Self>) {
        self.voice.update(cx, |v, cx| v.set_idle(cx));
        self.composer.update(cx, |c, cx| c.set_voice_idle(cx));
    }

    fn listen(&mut self, cx: &mut Context<Self>) {
        self.since = Instant::now();
        match self.face {
            Face::Bar => self.voice.update(cx, |v, cx| v.set_listening(cx)),
            Face::Composer => self.composer.update(cx, |c, cx| c.set_voice_listening(cx)),
        }
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        if self.installed {
            Self::cue(&self.start);
            self.listen(cx);
            return;
        }
        // First use: the model has to come first.
        self.show_setup(SetupPhase::Download(0.), cx);
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
                    if this.update(cx, |this, cx| this.show_setup(phase, cx)).is_err() {
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

    fn stop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        Self::cue(&self.stop);
        self.show_idle(cx);
        if self.face == Face::Composer {
            self.composer.update(cx, |c, cx| c.insert_transcript(TRANSCRIPT, window, cx));
        }
    }

    fn forget(&mut self, cx: &mut Context<Self>) {
        self.setup = None;
        self.installed = false;
        self.show_idle(cx);
        cx.notify();
    }

    /// Feeds the bars a made-up voice while listening.
    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.mode(cx) == VoiceMode::Listening {
            let level = self.level();
            match self.face {
                Face::Bar => self.voice.update(cx, |v, cx| v.set_level(level, cx)),
                Face::Composer => self.composer.update(cx, |c, cx| c.set_voice_level(level, cx)),
            }
            cx.notify();
        }
    }

    fn level(&self) -> f32 {
        level(self.since.elapsed().as_secs_f32())
    }
}

impl Render for VoiceStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let listening = self.mode(cx) == VoiceMode::Listening;
        let level = if listening { self.level() } else { 0. };
        let forget = cx.listener(|this, _, _, cx| this.forget(cx));
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .max_w(px(560.))
            .child(self.voice.clone())
            .child(self.composer.clone())
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
