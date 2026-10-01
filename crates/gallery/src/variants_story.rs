//! The "Variants" story: designs for three controls, each in atelier's look (our tokens, sizes and radii,
//! borderless) with the motion running, so Alex can pick. The toggle and tab designs are the parts the real app
//! shows through Settings, "Design preview" (`atelier_ui::design_preview`): there is no second copy.
//!
//! 1. The spinner: A the old glyph, B the ring, C three dots, D a bar sweep. Each 14px, at the text size.
//! 2. The editor tabs: A a chip, B a chip with a 2px bottom line, C a text tab with an under-dot, D a tab with a
//!    left accent tick. The marker glides between the tabs on the tabs' own spring.
//!
//! `VARIANTS_GROUP=spinner|tabs` shows one group alone, for a screenshot.
use std::time::Duration;

use atelier_ui::{
    ActiveTheme, FileIcon, Icon, IconName, Tab, Tabs,
    design_preview,
    motion::duration,
    typography::TextSize,
};
use gpui_kit::{
    Animation, AnimationExt, AnyElement, App, Context, ElementId, IntoElement, ParentElement, Render, Styled, Window, div, px,
};

pub struct VariantsStory {
    tabs: [usize; 4],
    group: Option<String>,
}

impl VariantsStory {
    pub fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        Self { tabs: [0; 4], group: std::env::var("VARIANTS_GROUP").ok() }
    }

    fn shows(&self, group: &str) -> bool {
        self.group.as_deref().is_none_or(|g| g == group)
    }
}

/// A control with its letter and a few words under it.
fn labelled(letter: &str, words: &str, theme: &atelier_ui::Theme, control: impl IntoElement) -> AnyElement {
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

fn group(title: &str, theme: &atelier_ui::Theme, items: Vec<AnyElement>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(div().text_size(TextSize::Sm.font_size()).font_weight(gpui_kit::FontWeight::MEDIUM).text_color(theme.foreground).child(title.to_string()))
        .child(div().flex().flex_wrap().gap(px(32.)).children(items))
}

impl Render for VariantsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let this = cx.entity();
        let mut groups: Vec<AnyElement> = Vec::new();

        if self.shows("spinner") {
            let words = |t: &str| div().text_size(TextSize::Sm.font_size()).text_color(theme.muted_foreground).child(t.to_string());
            let row = |mark: AnyElement| div().flex().items_center().gap(px(8.)).child(mark).child(words("Reading…"));
            groups.push(
                group(
                    "The spinner",
                    &theme,
                    vec![
                        labelled("A", "the old glyph", &theme, row(old_glyph(&theme))),
                        labelled("B", "the ring", &theme, row(atelier_ui::spinner::Spinner::new("vs-b").size(px(14.)).color(theme.muted_foreground).into_any_element())),
                        labelled("C", "three dots", &theme, row(dots(&theme))),
                        labelled("D", "a bar sweep", &theme, row(sweep(&theme))),
                    ],
                )
                .into_any_element(),
            );
        }

        if self.shows("tabs") {
            let names = ["main.rs", "lib.rs", "Cargo.toml"];
            let bar = |i: usize| {
                let pick = {
                    let this = this.clone();
                    move |tab: usize, _: &mut Window, cx: &mut App| {
                        this.update(cx, |s, cx| {
                            s.tabs[i] = tab;
                            cx.notify();
                        })
                    }
                };
                Tabs::new(ElementId::Integer(2000 + i as u64), design_preview::tab_variant(i), names.map(|n| Tab::new(n).leading(FileIcon::file(n).size(px(14.)))), Some(self.tabs[i])).on_select(pick)
            };
            groups.push(
                group(
                    "The editor tabs",
                    &theme,
                    (0..4).map(|i| labelled(["A", "B", "C", "D"][i], design_preview::TABS_DESIGNS[i], &theme, bar(i))).collect(),
                )
                .into_any_element(),
            );
        }

        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(40.))
            .p(px(32.))
            .bg(theme.background)
            .text_color(theme.foreground)
            .children(groups)
    }
}

/// The old spinner: the Material "progress" glyph, turning once a second.
fn old_glyph(theme: &atelier_ui::Theme) -> AnyElement {
    Icon::new(IconName::Progress)
        .size(px(14.))
        .color(theme.muted_foreground)
        .with_animation("vs-a", Animation::new(duration::SPIN).repeat(), |icon, t| icon.turn(t))
        .into_any_element()
}

/// Three dots that rise and fall in turn.
fn dots(theme: &atelier_ui::Theme) -> AnyElement {
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
fn sweep(theme: &atelier_ui::Theme) -> AnyElement {
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
