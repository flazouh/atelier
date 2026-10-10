//! The Usage view of the shell: the sources in the sidebar, as the other lenses fill theirs, and the detail of the one
//! chosen on the Code lens's card. The page is one for the window (the accounts are the person's, not a project's), made
//! the first time the view is in front. What it shows is written in `usage_view`; the shell only places it.

use gpui_kit::{AnyElement, Context, Window, div, prelude::*};

use super::{structs::Shell, view::ShellView};
use crate::{slots::Host, usage_view::UsagePage};

impl Shell {
    /// Shows the Usage view.
    pub(super) fn show_usage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let before = self.usage.is_some();
        self.ensure_usage(cx);
        // A page made earlier shows what it read then: the logs are read again each time the view comes in front.
        if let Some(page) = self.usage.clone().filter(|_| before && self.view != ShellView::Usage) {
            page.update(cx, |page, cx| page.refresh(cx));
        }
        self.show_view(ShellView::Usage, window, cx);
        cx.notify();
    }

    /// Makes the page when there is none.
    pub(super) fn ensure_usage(&mut self, cx: &mut Context<Self>) {
        if self.usage.is_none() {
            let host = Host::new(cx.weak_entity(), self.vitals.clone());
            self.usage = Some(cx.new(|cx| UsagePage::new(host, cx)));
        }
    }

    /// The Usage view's sidebar: all accounts, then each provider's with its meters.
    pub(super) fn usage_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.usage.clone() {
            Some(page) => page.update(cx, |page, cx| page.sidebar(cx)),
            None => div().into_any_element(),
        }
    }

    /// The Usage view's main area: the detail on the card the Code lens uses.
    pub(super) fn usage_main(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.usage.clone() {
            Some(page) => {
                let detail = page.update(cx, |page, cx| page.main(cx));
                self.code_card(detail, cx)
            }
            None => div().into_any_element(),
        }
    }
}
