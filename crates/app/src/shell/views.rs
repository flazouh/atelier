//! The view a module opened over the window, as the slots built it: one at a time, until it asks to close.
use gpui_kit::Context;

use super::structs::Shell;
use super::view::ShellView;
use crate::slots::{Host, Slots};

impl Shell {
    /// Opens the view of the [`RailView`](crate::slots::RailView) registered as `id`. Nothing happens for an id nobody
    /// registered, so a module that is not in the app leaves its doors dead and nothing else.
    pub fn open_view(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(view) = cx.global::<Slots>().view(id).cloned() else {
            return;
        };
        // Usage and Bots are lenses of the shell, not layers over it: the next frame, which has a window, shows them.
        let lens = match id {
            "usage" => Some(ShellView::Usage),
            "bots" => Some(ShellView::Bots),
            _ => None,
        };
        if let Some(lens) = lens {
            self.lens_asked = Some(lens);
            cx.notify();
            return;
        }
        let host = Host::new(cx.weak_entity(), self.vitals.clone());
        let page = (view.open)(&host, cx);
        self.opened = Some((view.id, page));
        cx.notify();
    }

}
