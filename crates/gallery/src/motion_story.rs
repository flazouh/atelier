//! The "Motion" story: the components ported from beui.dev's `motion/` and `blocks/`, each in every state it
//! has. `MOTION_PART=<name>` shows one alone, at the top left of the page, so a screenshot of it can be laid
//! beside the web demo's (`~/shots/beui/<name>-compare.png`). Without it, every part is listed.
use gpui_kit::AppContext as _;
use beui::{ActiveTheme, Checkbox, ColorSelector, MultiOption, MultiSelect, RangeSlider, Swatch};
use gpui_kit::{
    AnyElement, Context, Entity, Hsla, IntoElement, ParentElement, Render, Rgba, SharedString, Styled, Window, div, px,
};

/// The demo's accents, from `color-selector.preview.tsx`, as red, green and blue bytes: they are the
/// user's data, not the UI's colours.
const ACCENTS: [(&str, [u8; 3], &str); 8] = [
    ("blue", [52, 120, 246], "Blue"),
    ("purple", [146, 112, 232], "Purple"),
    ("pink", [230, 106, 164], "Pink"),
    ("red", [229, 86, 86], "Red"),
    ("orange", [237, 145, 65], "Orange"),
    ("amber", [229, 182, 60], "Amber"),
    ("green", [101, 166, 90], "Green"),
    ("teal", [22, 157, 131], "Teal"),
];

/// The demo's teams, from `multi-select.preview.tsx`; the dots are Tailwind's rose, sky, amber, violet, emerald and slate 500.
fn teams() -> Vec<MultiOption> {
    let dot = |r: u8, g: u8, b: u8| Hsla::from(Rgba { r: r as f32 / 255., g: g as f32 / 255., b: b as f32 / 255., a: 1. });
    let product = "Product teams";
    let business = "Business teams";
    vec![
        MultiOption::new("design", "Design").group(product).dot(dot(244, 63, 94)),
        MultiOption::new("engineering", "Engineering").group(product).dot(dot(14, 165, 233)),
        MultiOption::new("product", "Product").group(product).dot(dot(245, 158, 11)),
        MultiOption::new("research", "Research").group(product).dot(dot(139, 92, 246)),
        MultiOption::new("marketing", "Marketing").group(business).dot(dot(16, 185, 129)),
        MultiOption::new("operations", "Operations").group(business).dot(dot(100, 116, 139)),
    ]
}

fn accents() -> Vec<Swatch> {
    ACCENTS
        .iter()
        .map(|(value, [r, g, b], label)| {
            let color = Hsla::from(Rgba { r: *r as f32 / 255., g: *g as f32 / 255., b: *b as f32 / 255., a: 1. });
            Swatch::new(*value, color, *label)
        })
        .collect()
}

pub struct MotionStory {
    part: Option<String>,
    teams: Entity<MultiSelect>,
    accent: SharedString,
    /// Owned by the "every state" rows below.
    second: SharedString,
    third: SharedString,
    terms: bool,
    updates: bool,
    all: bool,
    level: f32,
    fine: f32,
}

impl MotionStory {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // `MOTION_PICK=2,133,247` puts a primary colour in force, to set the web demo's blue beside ours.
        if let Ok(pick) = std::env::var("MOTION_PICK") {
            let bytes: Vec<f32> = pick.split(',').filter_map(|n| n.trim().parse::<f32>().ok()).collect();
            if let [r, g, b] = bytes[..] {
                beui::theme::set_pick(Some(Hsla::from(Rgba { r: r / 255., g: g / 255., b: b / 255., a: 1. })), cx);
            }
        }
        let teams = cx.new(|cx| MultiSelect::new("teams", teams(), window, cx).placeholder("Choose teams").empty("No teams found.").with_values(["design", "engineering"]));
        Self { teams, part: std::env::var("MOTION_PART").ok(), accent: "blue".into(), second: "green".into(), third: "pink".into(), terms: true, updates: false, all: false, level: 40., fine: 2.5 }
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
        if self.shows("checkbox") {
            let this = cx.entity().downgrade();
            let flip = |field: fn(&mut MotionStory) -> &mut bool| {
                let this = this.clone();
                move |v: bool, _: &mut Window, cx: &mut gpui_kit::App| {
                    this.update(cx, |s, cx| {
                        *field(s) = v;
                        cx.notify();
                    })
                    .ok();
                }
            };
            let demo = div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(Checkbox::new("terms", self.terms).label("Accept terms and conditions").on_change(flip(|s| &mut s.terms)))
                .child(Checkbox::new("updates", self.updates).label("Email me product updates").on_change(flip(|s| &mut s.updates)))
                .child(Checkbox::new("partial", true).indeterminate(true).label("Select all (partial)").on_change(|_, _, _| {}))
                .child(Checkbox::new("off", true).disabled(true).label("Disabled").on_change(|_, _, _| {}));
            if alone {
                parts.push(demo.into_any_element());
            } else {
                parts.push(section("Checkbox: the demo", &theme, demo));
                parts.push(section(
                    "Checkbox: every state",
                    &theme,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .child(Checkbox::new("s1", false).label("Unchecked").on_change(|_, _, _| {}))
                        .child(Checkbox::new("s2", true).label("Checked").on_change(|_, _, _| {}))
                        .child(Checkbox::new("s3", false).indeterminate(true).label("Partial").on_change(|_, _, _| {}))
                        .child(Checkbox::new("s4", false).disabled(true).label("Unchecked, disabled").on_change(|_, _, _| {}))
                        .child(Checkbox::new("s5", true).disabled(true).label("Checked, disabled").on_change(|_, _, _| {}))
                        .child(Checkbox::new("s6", false).indeterminate(true).disabled(true).label("Partial, disabled").on_change(|_, _, _| {}))
                        .child(Checkbox::new("s7", true).on_change({
                            let this = this.clone();
                            move |v, _, cx| {
                                this.update(cx, |s, cx| {
                                    s.all = v;
                                    cx.notify();
                                })
                                .ok();
                            }
                        })),
                ));
            }
        }
        if self.shows("range-slider") {
            let this = cx.entity().downgrade();
            let set = |field: fn(&mut MotionStory) -> &mut f32| {
                let this = this.clone();
                move |v: f32, _: &mut Window, cx: &mut gpui_kit::App| {
                    this.update(cx, |s, cx| {
                        *field(s) = v;
                        cx.notify();
                    })
                    .ok();
                }
            };
            let demo = div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .w_full()
                .max_w(px(384.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .text_color(theme.muted_foreground)
                        .child("Drag the handle")
                        .child(div().text_color(theme.foreground).child(format!("{}", self.level))),
                )
                .child(RangeSlider::new("range", self.level).step(5.).on_change(set(|s| &mut s.level)));
            if alone {
                parts.push(demo.into_any_element());
            } else {
                parts.push(section("Range slider: the demo", &theme, demo));
                parts.push(section(
                    "Range slider: no dots, the whole range, a fine step, disabled, and a step that does not divide the range",
                    &theme,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .w(px(384.))
                        .child(RangeSlider::new("r1", self.level).ticks(false).on_change(set(|s| &mut s.level)))
                        .child(RangeSlider::new("r2", self.fine).range(0., 5.).step(0.5).on_change(set(|s| &mut s.fine)))
                        .child(RangeSlider::new("r3", 70.).step(10.).disabled(true))
                        .child(RangeSlider::new("r4", 10.).range(0., 10.).step(4.).on_change(|_, _, _| {}))
                        .child(RangeSlider::new("r5", 0.).step(10.).on_change(|_, _, _| {})),
                ));
            }
        }
        if self.shows("multi-select") {
            let demo = div().w(px(384.)).child(self.teams.clone());
            parts.push(if alone { demo.into_any_element() } else { section("Multi select: the demo", &theme, demo) });
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
