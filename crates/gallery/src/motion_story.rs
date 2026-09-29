//! The "Motion" story: the components ported from beui.dev's `motion/` and `blocks/`, each in every state it
//! has. `MOTION_PART=<name>` shows one alone, at the top left of the page, so a screenshot of it can be laid
//! beside the web demo's (`~/shots/beui/<name>-compare.png`). Without it, every part is listed.
use beui::{ActiveTheme, ColorSelector, Swatch};
use gpui_kit::{
    AnyElement, Context, Hsla, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, px, rgb,
};

/// The demo's accents, from `color-selector.preview.tsx`.
const ACCENTS: [(&str, u32, &str); 8] = [
    ("blue", 0x3478f6, "Blue"),
    ("purple", 0x9270e8, "Purple"),
    ("pink", 0xe66aa4, "Pink"),
    ("red", 0xe55656, "Red"),
    ("orange", 0xed9141, "Orange"),
    ("amber", 0xe5b63c, "Amber"),
    ("green", 0x65a65a, "Green"),
    ("teal", 0x169d83, "Teal"),
];

fn accents() -> Vec<Swatch> {
    ACCENTS.iter().map(|(value, hex, label)| Swatch::new(*value, Hsla::from(rgb(*hex)), *label)).collect()
}

pub struct MotionStory {
    part: Option<String>,
    accent: SharedString,
    /// Owned by the "every state" rows below.
    second: SharedString,
    third: SharedString,
}

impl MotionStory {
    pub fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        Self { part: std::env::var("MOTION_PART").ok(), accent: "blue".into(), second: "green".into(), third: "pink".into() }
    }

    fn shows(&self, name: &str) -> bool {
        self.part.as_deref().is_none_or(|p| p == name)
    }
}

fn section(title: &'static str, theme: &beui::Theme, body: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(title))
        .child(body)
        .into_any_element()
}

impl Render for MotionStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let alone = self.part.is_some();
        let mut parts: Vec<AnyElement> = Vec::new();
        if self.shows("color-selector") {
            let this = cx.entity().downgrade();
            let demo = ColorSelector::new("accent", accents()).label("Accent").value(Some(self.accent.clone())).on_change({
                let this = this.clone();
                move |v, _, cx| {
                    let v = v.clone();
                    this.update(cx, |s, cx| {
                        s.accent = v;
                        cx.notify();
                    })
                    .ok();
                }
            });
            if alone {
                parts.push(demo.into_any_element());
            } else {
                parts.push(section("Color selector: the demo", &theme, demo));
                let mut off = accents();
                off[2] = off[2].clone().disabled(true);
                off[5] = off[5].clone().disabled(true);
                let second = ColorSelector::new("second", off).label("Some swatches cannot be chosen").value(Some(self.second.clone())).on_change({
                    let this = this.clone();
                    move |v, _, cx| {
                        let v = v.clone();
                        this.update(cx, |s, cx| {
                            s.second = v;
                            cx.notify();
                        })
                        .ok();
                    }
                });
                parts.push(section("Color selector: disabled swatches", &theme, second));
                parts.push(section(
                    "Color selector: the whole group disabled",
                    &theme,
                    ColorSelector::new("third", accents()).label("Accent").value(Some("pink".into())).disabled(true),
                ));
                parts.push(section(
                    "Color selector: nothing chosen, in a narrow column",
                    &theme,
                    div().w(px(160.)).child(ColorSelector::new("fourth", accents()).label("Accent").on_change({
                        let this = this.clone();
                        move |v, _, cx| {
                            let v = v.clone();
                            this.update(cx, |s, cx| {
                                s.third = v;
                                cx.notify();
                            })
                            .ok();
                        }
                    })),
                ));
            }
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(28.))
            .p(px(24.))
            .bg(theme.background)
            .children(parts)
    }
}
