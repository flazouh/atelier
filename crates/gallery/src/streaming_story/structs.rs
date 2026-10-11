use std::time::Duration;

use atelier_ui::{
    ActiveTheme,
    agent_text::{AgentText, AgentTextStatus},
};
use gpui_kit::{Context, IntoElement, ParentElement, Render, Styled, Window, div, px};

use super::types::TOKENS;
use super::helpers::answer;

pub struct StreamingStory {
    pub(super) tokens: usize,
    pub(super) fade: bool,
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
            ),
        )
    }
}
