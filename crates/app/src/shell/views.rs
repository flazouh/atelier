//! The views the plugins registered, as the shell shows them. The shell knows none of them by name: it keeps one page
//! for each view, made the first time the view is in front, puts the page's sidebar in the sidebar and its main area on
//! the card the Code views use, and tells the page each time its view comes in front again.
use atelier_plugin::{Page, PluginPage};
use gpui_kit::{AnyElement, Context, Entity, Window, div, prelude::*};

use super::structs::Shell;
use super::view::ShellView;
use crate::slots::{Host, Slots};

impl Shell {
    /// Brings the view a plugin registered as `id` in front. Nothing happens for an id nobody registered, so a plugin
    /// that is not in the app leaves its doors dead and nothing else. There is no window here: the next frame, which has
    /// one, shows the view.
    pub fn open_view(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(view) = cx.global::<Slots>().view(id) else {
            return;
        };
        self.asked = Some(view.id);
        cx.notify();
    }

    /// Shows the view `id`, as a press on its entry does.
    pub(super) fn show_plugin(&mut self, id: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let made = self.pages.contains_key(id);
        let Some(page) = self.page(id, cx) else { return };
        // A page made earlier shows what it read then: it is told each time its view comes in front again.
        if made && self.view != ShellView::Plugin(id) {
            page.in_front_again(cx);
        }
        self.show_view(ShellView::Plugin(id), window, cx);
        cx.notify();
    }

    /// The page of the view `id`, made when there is none. No page for a view nobody registered: one kept from a
    /// registration that is gone is dropped.
    fn page(&mut self, id: &'static str, cx: &mut Context<Self>) -> Option<Page> {
        let Some(view) = cx.global::<Slots>().view(id) else {
            self.pages.remove(id);
            return None;
        };
        if let Some(page) = self.pages.get(id) {
            return Some(page.clone());
        }
        let view = view.clone();
        // The handle the page gets, over the app's own host.
        let host = atelier_plugin::Host::new(Host::new(cx.weak_entity(), self.vitals.clone()));
        let page = view.open(&host, cx);
        self.pages.insert(id, page.clone());
        Some(page)
    }

    /// Before a frame is laid out: the view a plugin asked for comes in front, and a plugin's view in front has its
    /// page (a window opens on the view it closed on). A view whose registration is gone gives way to Sessions.
    pub(super) fn settle_plugin_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.asked.take() {
            self.show_plugin(id, window, cx);
        }
        if let ShellView::Plugin(id) = self.view
            && self.page(id, cx).is_none()
        {
            self.show_view(ShellView::Sessions, window, cx);
        }
    }

    /// The sidebar of the view `id`, as its page draws it.
    pub(super) fn plugin_sidebar(&self, id: &str, cx: &mut Context<Self>) -> AnyElement {
        match self.pages.get(id).cloned() {
            Some(page) => page.sidebar(cx),
            None => div().into_any_element(),
        }
    }

    /// The main area of the view `id`: what its page draws, on the card the Code views use.
    pub(super) fn plugin_main(&self, id: &str, cx: &mut Context<Self>) -> AnyElement {
        match self.pages.get(id).cloned() {
            Some(page) => {
                let detail = page.main(cx);
                self.code_card(detail, cx)
            }
            None => div().into_any_element(),
        }
    }

    /// The page of the view `id` as the type its plugin made it, once the view was in front.
    pub(crate) fn plugin_page<P: PluginPage>(&self, id: &str) -> Option<Entity<P>> {
        self.pages.get(id)?.downcast()
    }
}
