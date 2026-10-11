use atelier_plugin::PluginPage;
use atelier_ui::{scale::px, typography::TextSize};
use gpui_kit::{AnyElement, Context, div, prelude::*};

use super::{consts::ROW_FACE, structs::BotsPage};
use crate::shell::nav_row_marked;

/// The bot library as the app draws it: the bots in the sidebar, and the profile of the one chosen in the main area.
impl PluginPage for BotsPage {
    /// The sidebar: its title, then one row per bot with its face, its name and its role.
    fn sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let column = div()
            .id("bots-sidebar")
            .debug_selector(|| "bots-sidebar".into())
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .gap(px(1.))
            .px(px(4.))
            .pb(px(8.))
            .child(
                div()
                    .px(px(10.))
                    .pt(px(10.))
                    .pb(px(8.))
                    .text_size(TextSize::Lg.font_size())
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child("Bots"),
            );
        let rows: Vec<AnyElement> = self
            .entries()
            .iter()
            .map(|entry| {
                let id = entry.bot.id.clone();
                let on = self.selected() == Some(&id);
                let mark = self.row_face(entry, cx).unwrap_or_else(|| div().into_any_element());
                let label = format!("{} · {}", entry.bot.name, entry.bot.role);
                let pick = cx.weak_entity();
                let name = format!("bot-row-{id}");
                let row = nav_row_marked(name.clone(), on, mark, ROW_FACE, label.into(), None, cx)
                    .on_click(move |_, _, cx| drop(pick.update(cx, |page, cx| page.select(id.clone(), cx))));
                crate::control::marked_named(name, row)
            })
            .collect();
        // A folder that could not be read leaves the list empty; the main area says why.
        column.children(rows).into_any_element()
    }

    /// The main area: the profile, which the page draws itself.
    fn main(&mut self, cx: &mut Context<Self>) -> AnyElement {
        BotsPage::main(self, cx)
    }

    /// The folder is read again each time the view comes in front, so an edit on disk shows.
    fn in_front_again(&mut self, cx: &mut Context<Self>) {
        self.refresh(cx);
    }
}
