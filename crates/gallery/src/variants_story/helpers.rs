use std::time::Duration;

use atelier_ui::{Icon, IconName, motion::duration, typography::TextSize};
use gpui_kit::{
    Animation, AnimationExt, AnyElement, ElementId, IntoElement, ParentElement, Styled, div, px,
};

/// A control with its letter and a few words under it.
pub(super) fn labelled(letter: &str, words: &str, theme: &atelier_ui::Theme, control: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(div().flex().items_center().child(control))
        .child(
            div()
                .flex()
                .gap(px(8.))
                .text_size(TextSize::Xs.font_size())
                .text_color(theme.muted_foreground)
                .child(div().font_weight(gpui_kit::FontWeight::SEMIBOLD).text_color(theme.foreground).child(letter.to_string()))
                .child(words.to_string()),
        )
        .into_any_element()
}

pub(super) fn group(title: &str, theme: &atelier_ui::Theme, items: Vec<AnyElement>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(div().text_size(TextSize::Sm.font_size()).font_weight(gpui_kit::FontWeight::MEDIUM).text_color(theme.foreground).child(title.to_string()))
        .child(div().flex().flex_wrap().gap(px(32.)).children(items))
}

/// The old spinner: the Material "progress" glyph, turning once a second.
pub(super) fn old_glyph(theme: &atelier_ui::Theme) -> AnyElement {
    Icon::new(IconName::Progress)
        .size(px(14.))
        .color(theme.muted_foreground)
        .with_animation("vs-a", Animation::new(duration::SPIN).repeat(), |icon, t| icon.turn(t))
        .into_any_element()
}

/// Three dots that rise and fall in turn.
pub(super) fn dots(theme: &atelier_ui::Theme) -> AnyElement {
    let ink = theme.muted_foreground;
    div()
        .flex()
        .items_center()
        .gap(px(3.))
        .h(px(14.))
        .children((0..3).map(move |i| {
            div().size(px(4.)).rounded_full().bg(ink).with_animation(
                ElementId::Integer(1000 + i as u64),
                Animation::new(Duration::from_millis(900)).repeat(),
                move |dot, t| {
                    let phase = (t + 1. - i as f32 * 0.2) % 1.;
                    let wave = (1. - (phase * 2. - 1.).abs()).max(0.);
                    dot.opacity(0.25 + 0.75 * wave)
                },
            )
        }))
        .into_any_element()
}

/// A 14px bar with a short piece that sweeps across it.
pub(super) fn sweep(theme: &atelier_ui::Theme) -> AnyElement {
    let ink = theme.muted_foreground;
    div()
        .relative()
        .flex_none()
        .w(px(14.))
        .h(px(3.))
        .overflow_hidden()
        .rounded_full()
        .bg(ink.opacity(0.2))
        .child(div().absolute().top_0().h(px(3.)).w(px(6.)).rounded_full().bg(ink).with_animation(
            "vs-sweep",
            Animation::new(Duration::from_millis(1100)).repeat(),
            |piece, t| piece.left(px(-6. + 20. * t)),
        ))
        .into_any_element()
}
