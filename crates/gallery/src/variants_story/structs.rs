use atelier_ui::{ActiveTheme, FileIcon, Tab, Tabs, design_preview, typography::TextSize};
use gpui_kit::{
    AnyElement, App, Context, ElementId, IntoElement, ParentElement, Render, Styled, Window,
    div, px,
};

use super::helpers::{dots, group, labelled, old_glyph, sweep};

pub struct VariantsStory {
    pub(super) tabs: [usize; 4],
    pub(super) group: Option<String>,
}

impl VariantsStory {
    pub fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        Self { tabs: [0; 4], group: std::env::var("VARIANTS_GROUP").ok() }
    }

    pub(super) fn shows(&self, group: &str) -> bool {
        self.group.as_deref().is_none_or(|g| g == group)
    }
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
