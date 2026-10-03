use std::time::{Duration, Instant};

use atelier_ui::{ActiveTheme, GlyphText};
use gpui_kit::{Context, HighlightStyle, Hsla, IntoElement, ParentElement, Render, Styled, Task, Window, div, px};

use super::data::TIMELINE;
use super::model::{Track, Way};

/// After the last partial, the final words sit this long before the loop starts over.
const HOLD: u64 = 3500;

/// `WORDS_FROM` and `WORDS_TO` (ms into the press) loop just a stretch of it, to look closely at one moment.
fn window() -> (u64, u64) {
    let get = |k: &str, d: u64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    (get("WORDS_FROM", 1800), get("WORDS_TO", TIMELINE.last().map_or(0, |(t, _)| *t)))
}

pub struct WordsStory {
    tracks: Vec<Track>,
    began: Instant,
    last: Instant,
    fed: usize,
    speed: f32,
    _ticker: Task<()>,
}

impl WordsStory {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        let now = Instant::now();
        let speed = std::env::var("WORDS_SPEED").ok().and_then(|v| v.parse().ok()).unwrap_or(1.);
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(16)).await;
                if this.update(cx, |s, cx| s.tick(cx)).is_err() {
                    return;
                }
            }
        });
        Self { tracks: Way::ALL.iter().map(|w| Track::new(*w, now)).collect(), began: now, last: now, fed: 0, speed, _ticker: ticker }
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let (from, to) = window();
        let elapsed = (now.duration_since(self.began).as_millis() as f32 * self.speed) as u64 + from;
        let hold = std::env::var("WORDS_HOLD").ok().and_then(|v| v.parse().ok()).unwrap_or(HOLD);
        if elapsed >= to + hold {
            eprintln!("LOOP {}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis()));
            self.tracks = Way::ALL.iter().map(|w| Track::new(*w, now)).collect();
            self.began = now;
            self.fed = 0;
            // What was said before the stretch is already on the page.
            while let Some((_, words)) = TIMELINE.get(self.fed).filter(|(at, _)| *at < from) {
                for t in &mut self.tracks {
                    t.observe(words, now);
                    t.settle();
                }
                self.fed += 1;
            }
            return;
        }
        while let Some((at, words)) = TIMELINE.get(self.fed).filter(|(at, _)| *at <= elapsed) {
            let _ = at;
            if *at > to {
                break;
            }
            for t in &mut self.tracks {
                t.observe(words, now);
            }
            self.fed += 1;
        }
        let dt = now.duration_since(self.last);
        self.last = now;
        for t in &mut self.tracks {
            t.step(dt, now);
        }
        cx.notify();
    }
}

/// `over` laid on `under` at strength `k`.
fn lay(under: Hsla, over: Hsla, k: f32) -> Hsla {
    under.blend(over.opacity(k.clamp(0., 1.)))
}

impl Render for WordsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let surface = theme.card;
        let cards = self.tracks.iter().map(|track| {
            let highlights = track.words.iter().enumerate().take_while(|(_, w)| w.range.end <= track.shown_text().len()).map(|(_, w)| {
                let ink = lay(theme.foreground, theme.primary, w.tint);
                let ink = lay(ink, surface, 1. - w.a);
                (w.range.clone(), HighlightStyle { color: Some(ink), ..Default::default() })
            });
            let text = GlyphText::new(track.shown_text().to_string()).highlights(highlights.collect::<Vec<_>>());
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .w(px(400.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().text_size(px(13.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).child(track.way.name()))
                        .child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(track.way.gist())),
                )
                .child(
                    div()
                        .h(px(212.))
                        .p(px(16.))
                        .rounded(px(14.))
                        .bg(surface)
                        .border_1()
                        .border_color(theme.divider)
                        .text_size(px(14.))
                        .line_height(px(22.))
                        .text_color(theme.foreground)
                        .child(text),
                )
        });
        div()
            .w_full()
            .flex()
            .flex_wrap()
            .content_start()
            .gap(px(24.))
            .children(cards)
    }
}
