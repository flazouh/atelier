//! The Bots view of the shell: the bots in the sidebar, as the other lenses fill theirs, and the profile of the one
//! chosen on the Code lens's card. The page is one for the window (the bots are the person's, not a project's), made
//! the first time the view is in front. What it shows is written in `bots_view`; the shell only places it.

use atelier_ui::{ActiveTheme, scale::px, typography::TextSize};
use gpui_kit::{AnyElement, Context, Window, div, prelude::*};

use super::{lens::nav_row_marked, structs::Shell, view::ShellView};
use crate::bots_view::{BotsPage, consts::ROW_FACE};

impl Shell {
    /// Shows the Bots view.
    pub(super) fn show_bots(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let before = self.bots.is_some();
        self.ensure_bots(cx);
        // A page made earlier shows what it read then: the folder is read again each time the view comes in front.
        if let Some(page) = self.bots.clone().filter(|_| before && self.view != ShellView::Bots) {
            page.update(cx, |page, cx| page.refresh(cx));
        }
        self.show_view(ShellView::Bots, window, cx);
        cx.notify();
    }

    /// Makes the page when there is none.
    pub(super) fn ensure_bots(&mut self, cx: &mut Context<Self>) {
        if self.bots.is_none() {
            let root = self.bots_root.clone();
            self.bots = Some(cx.new(|cx| BotsPage::new(root, cx)));
        }
    }

    /// The Bots view's sidebar: its title, then one row per bot with its face, its name and its role.
    pub(super) fn bots_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(page) = self.bots.clone() else { return div().into_any_element() };
        let theme = cx.theme().clone();
        let mut column = div()
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
        let rows: Vec<AnyElement> = page.update(cx, |page, cx| {
            page.entries()
                .iter()
                .map(|entry| {
                    let id = entry.bot.id.clone();
                    let on = page.selected() == Some(&id);
                    let mark = page.row_face(entry, cx).unwrap_or_else(|| div().into_any_element());
                    let label = format!("{} · {}", entry.bot.name, entry.bot.role);
                    let pick = cx.weak_entity();
                    nav_row_marked(format!("bot-row-{id}"), on, mark, ROW_FACE, label.into(), None, cx)
                        .on_click(move |_, _, cx| drop(pick.update(cx, |page, cx| page.select(id.clone(), cx))))
                        .into_any_element()
                })
                .collect()
        });
        let empty = rows.is_empty();
        column = column.children(rows);
        if empty && let Some(error) = page.read(cx).error() {
            column = column.child(
                div().px(px(8.)).pt(px(10.)).text_size(TextSize::Xs.font_size()).text_color(theme.danger).child(error.to_string()),
            );
        }
        column.into_any_element()
    }

    /// The Bots view's main area: the profile on the card the Code lens uses.
    pub(super) fn bots_main(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.bots.clone() {
            Some(page) => {
                let detail = page.update(cx, |page, cx| page.main(cx));
                self.code_card(detail, cx)
            }
            None => div().into_any_element(),
        }
    }
}
