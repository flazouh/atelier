//! The "Variants" story: designs for three controls, each in lathe's look (our tokens, sizes and radii,
//! borderless) with the motion running, so Alex can pick. No app code uses any of this.
//!
//! 1. The grouping toggle: A the old toggle button, B a pressed ghost with a check, C a compact 20px switch,
//!    D a segmented Off / On.
//! 2. The spinner: A the old glyph, B the ring, C three dots, D a bar sweep. Each 14px, at the text size.
//! 3. The editor tabs: A the old pill with no line, B a pill with a 2px bottom line, C a text tab with an
//!    under-dot, D a tab with a left accent tick. The marker glides between the tabs on the tabs' own spring.
//!
//! `VARIANTS_GROUP=toggle|spinner|tabs` shows one group alone, for a screenshot.
use std::{rc::Rc, time::Duration};

use beui::{
    ActiveTheme, Button, ButtonSize, ButtonVariant, FileIcon, Icon, IconName, Segment, Segmented, Switch,
    motion::{Animated, FrameClock, duration},
    tabs::GLIDE,
    theme::radius,
    typography::TextSize,
};
use gpui_kit::{
    Animation, AnimationExt, AnyElement, App, Bounds, Context, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels,
    Render, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, canvas, div, prelude::FluentBuilder, px,
};


pub struct VariantsStory {
    toggles: [bool; 4],
    tabs: [usize; 4],
    group: Option<String>,
}

impl VariantsStory {
    pub fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        Self { toggles: [true, true, true, true], tabs: [0, 0, 0, 0], group: std::env::var("VARIANTS_GROUP").ok() }
    }

    fn shows(&self, group: &str) -> bool {
        self.group.as_deref().is_none_or(|g| g == group)
    }
}

/// A control with its letter and a few words under it.
fn labelled(letter: &str, words: &str, theme: &beui::Theme, control: impl IntoElement) -> AnyElement {
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

fn group(title: &str, theme: &beui::Theme, items: Vec<AnyElement>) -> impl IntoElement {
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

        if self.shows("toggle") {
            let flip = |i: usize| {
                let this = this.clone();
                move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
                    this.update(cx, |s, cx| {
                        s.toggles[i] = !s.toggles[i];
                        cx.notify();
                    })
                }
            };
            let set = |i: usize| {
                let this = this.clone();
                move |on: bool, _: &mut Window, cx: &mut App| {
                    this.update(cx, |s, cx| {
                        s.toggles[i] = on;
                        cx.notify();
                    })
                }
            };
            // Each design in both states: the first row is live (a press flips it), the second is the other state, still.
            let build = |i: usize, on: bool, live: bool| -> AnyElement {
                let id = |name: &str| SharedString::from(format!("vt-{name}-{}", if live { "live" } else { "still" }));
                match i {
                    0 => Button::new(id("a"))
                        .label("Group by project")
                        .variant(if on { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                        .size(ButtonSize::Sm)
                        .when(live, |b| b.on_click(flip(0)))
                        .into_any_element(),
                    1 => Button::new(id("b"))
                        .label("Group by project")
                        .variant(if on { ButtonVariant::Secondary } else { ButtonVariant::Ghost })
                        .size(ButtonSize::Sm)
                        .when(on, |b| b.icon(IconName::Check))
                        .when(live, |b| b.on_click(flip(1)))
                        .into_any_element(),
                    2 => Switch::new(id("c"), on)
                        .compact(true)
                        .label("Group by project")
                        .when(live, |s| s.on_change(set(2)))
                        .into_any_element(),
                    _ => div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().text_size(TextSize::Sm.font_size()).text_color(theme.foreground).child("Group by project"))
                        .child(
                            Segmented::new(id("d"), [Segment::new("Off"), Segment::new("On")], usize::from(on)).when(live, |g| {
                                let set = set(3);
                                g.on_change(move |i, window, cx| set(i == 1, window, cx))
                            }),
                        )
                        .into_any_element(),
                }
            };
            let both = |i: usize, letter: &str, words: &str| {
                let now = self.toggles[i];
                labelled(
                    letter,
                    words,
                    &theme,
                    div().flex().flex_col().gap(px(8.)).child(build(i, now, true)).child(build(i, !now, false)),
                )
            };
            groups.push(
                group(
                    "The grouping toggle (first row: live, second row: the other state)",
                    &theme,
                    vec![
                        both(0, "A", "the old toggle button"),
                        both(1, "B", "a pressed ghost with a check"),
                        both(2, "C", "a compact 20px switch"),
                        both(3, "D", "a segmented Off / On"),
                    ],
                )
                .into_any_element(),
            );
        }

        if self.shows("spinner") {
            let words = |t: &str| div().text_size(TextSize::Sm.font_size()).text_color(theme.muted_foreground).child(t.to_string());
            let row = |mark: AnyElement| div().flex().items_center().gap(px(8.)).child(mark).child(words("Reading…"));
            groups.push(
                group(
                    "The spinner",
                    &theme,
                    vec![
                        labelled("A", "the old glyph", &theme, row(old_glyph(&theme))),
                        labelled("B", "the ring", &theme, row(beui::spinner::Spinner::new("vs-b").size(px(14.)).color(theme.muted_foreground).into_any_element())),
                        labelled("C", "three dots", &theme, row(dots(&theme))),
                        labelled("D", "a bar sweep", &theme, row(sweep(&theme))),
                    ],
                )
                .into_any_element(),
            );
        }

        if self.shows("tabs") {
            let pick = |i: usize| {
                let this = this.clone();
                Rc::new(move |tab: usize, _: &mut Window, cx: &mut App| {
                    this.update(cx, |s, cx| {
                        s.tabs[i] = tab;
                        cx.notify();
                    })
                })
            };
            let names = ["main.rs", "lib.rs", "Cargo.toml"];
            let bar = |i: usize, marker: Marker| TabBar { id: ("vb", i).into(), marker, names: names.to_vec(), selected: self.tabs[i], on_pick: pick(i) };
            groups.push(
                group(
                    "The editor tabs",
                    &theme,
                    vec![
                        labelled("A", "the old pill, no line", &theme, bar(0, Marker::Pill)),
                        labelled("B", "a pill with a 2px bottom line", &theme, bar(1, Marker::PillLine)),
                        labelled("C", "a text tab with an under-dot", &theme, bar(2, Marker::Dot)),
                        labelled("D", "a tab with a left accent tick", &theme, bar(3, Marker::Tick)),
                    ],
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
fn old_glyph(theme: &beui::Theme) -> AnyElement {
    Icon::new(IconName::Progress)
        .size(px(14.))
        .color(theme.muted_foreground)
        .with_animation("vs-a", Animation::new(duration::SPIN).repeat(), |icon, t| icon.turn(t))
        .into_any_element()
}

/// Three dots that rise and fall in turn.
fn dots(theme: &beui::Theme) -> AnyElement {
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
fn sweep(theme: &beui::Theme) -> AnyElement {
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

type Pick = Rc<dyn Fn(usize, &mut Window, &mut App)>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Marker {
    Pill,
    PillLine,
    Dot,
    Tick,
}

struct Glide {
    rects: Vec<Option<Bounds<Pixels>>>,
    list: Option<Bounds<Pixels>>,
    left: Animated,
    width: Animated,
    seeded: bool,
    clock: FrameClock,
}

#[derive(IntoElement)]
struct TabBar {
    id: ElementId,
    marker: Marker,
    names: Vec<&'static str>,
    selected: usize,
    on_pick: Pick,
}

impl RenderOnce for TabBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let reduce = cx.reduce_motion();
        let count = self.names.len();
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| Glide {
            rects: Vec::new(),
            list: None,
            left: Animated::new(GLIDE, 0.),
            width: Animated::new(GLIDE, 0.),
            seeded: false,
            clock: FrameClock::default(),
        });
        let (glide, moving) = state.update(cx, |s, _| {
            s.rects.resize(count, None);
            let target = s.rects.get(self.selected).copied().flatten().zip(s.list).map(|(r, l)| (f32::from(r.origin.x - l.origin.x), f32::from(r.size.width)));
            let mut moving = target.is_none();
            if let Some((x, w)) = target {
                if s.seeded {
                    s.left.set_target(x);
                    s.width.set_target(w);
                } else {
                    s.left = Animated::new(GLIDE, x);
                    s.width = Animated::new(GLIDE, w);
                    s.seeded = true;
                }
            }
            let dt = s.clock.tick();
            moving |= s.left.step(dt, reduce) | s.width.step(dt, reduce);
            if !moving {
                s.clock.rest();
            }
            (target.map(|_| (s.left.value(), s.width.value())), moving)
        });
        if moving {
            window.request_animation_frame();
        }
        let marker = glide.map(|(left, width)| match self.marker {
            Marker::Pill => div().absolute().top_0().h(px(28.)).left(px(left)).w(px(width)).rounded(radius::MD).bg(theme.card_strong),
            Marker::PillLine => div()
                .absolute()
                .top_0()
                .h(px(28.))
                .left(px(left))
                .w(px(width))
                .rounded(radius::MD)
                .bg(theme.card_strong)
                .child(div().absolute().left(px(10.)).right(px(10.)).bottom_0().h(px(2.)).rounded_full().bg(theme.foreground)),
            Marker::Dot => div().absolute().top(px(32.)).left(px(left + width / 2. - 2.)).size(px(4.)).rounded_full().bg(theme.foreground),
            Marker::Tick => div().absolute().top(px(7.)).left(px(left + 2.)).w(px(2.)).h(px(14.)).rounded_full().bg(theme.primary),
        });
        let report_list = {
            let state = state.clone();
            canvas(move |b, _, cx| state.update(cx, |s, _| s.list = Some(b)), |_, _, _, _| {}).absolute().inset_0()
        };
        let tabs: Vec<AnyElement> = self
            .names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let on = i == self.selected;
                let ink = if on { theme.foreground } else { theme.muted_foreground };
                let report = {
                    let state = state.clone();
                    canvas(move |b, _, cx| state.update(cx, |s, _| s.rects[i] = Some(b)), |_, _, _, _| {}).absolute().inset_0()
                };
                let pick = self.on_pick.clone();
                div()
                    .id(ElementId::from((self.id.clone(), SharedString::from(*name))))
                    .relative()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(6.))
                    .h(px(28.))
                    .px(px(10.))
                    .text_size(TextSize::Sm.font_size())
                    .text_color(ink)
                    .cursor_pointer()
                    .child(report)
                    .child(FileIcon::file(name).size(px(14.)))
                    .child(div().relative().child(name.to_string()))
                    .on_click(move |_, window, cx| pick(i, window, cx))
                    .into_any_element()
            })
            .collect();
        div()
            .relative()
            .flex()
            .items_center()
            .gap(px(4.))
            .pb(px(if self.marker == Marker::Dot { 8. } else { 0. }))
            .child(report_list)
            .children(marker)
            .children(tabs)
    }
}
