//! The "Streaming" story: an answer streams in at 60 tokens a second, over and over, in the app's own `AgentText`.
//! `STREAM_FADE=0` turns the tail's fade off, to set the old look beside the new. With `LATHE_FRAMES`-style timing the
//! frame cost of both is in `cargo test -p beui streaming_frame_cost -- --ignored --nocapture`.
use std::time::Duration;

use beui::{
    ActiveTheme,
    agent_text::{AgentText, AgentTextStatus},
};
use gpui_kit::{Context, IntoElement, ParentElement, Render, Styled, Window, div, px};

const WORDS: [&str; 12] = ["the", "build", "passed", "and", "every", "test", "ran", "without", "a", "single", "failure", "today"];
const TOKENS: usize = 340;

pub struct StreamingStory {
    tokens: usize,
    fade: bool,
}

/// The answer after `n` tokens: three words to a token, a blank line every 14 tokens.
fn answer(n: usize) -> String {
    let mut out = String::new();
    for i in 0..n {
        for k in 0..3 {
            out.push_str(WORDS[(i * 3 + k) % WORDS.len()]);
            out.push(' ');
        }
        if i % 14 == 13 {
            out.push_str("\n\n");
        }
    }
    out
}

impl StreamingStory {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        let fade = std::env::var("STREAM_FADE").map_or(true, |v| v != "0");
        cx.spawn(async move |this, cx| {
            loop {
                for n in 0..=TOKENS {
                    cx.background_executor().timer(Duration::from_millis(16)).await;
                    if this.update(cx, |s, cx| {
                        s.tokens = n;
                        cx.notify();
                    }).is_err() {
                        return;
                    }
                }
                cx.background_executor().timer(Duration::from_secs(3)).await;
            }
        })
        .detach();
        Self { tokens: 0, fade }
    }
}

impl Render for StreamingStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let done = self.tokens >= TOKENS;
        div().size_full().p(px(32.)).bg(theme.background).text_color(theme.foreground).child(
            div().w(px(640.)).child(
                AgentText::new("stream", answer(self.tokens))
                    .status(if done { AgentTextStatus::Complete } else { AgentTextStatus::Streaming })
                    .fade_tail(self.fade)
                    .fade_into(theme.background),
            ),
        )
    }
}
