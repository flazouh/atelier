//! The view a module opened over the window, as the slots built it: one at a time, until it asks to close.
use gpui_kit::Context;

use super::structs::Shell;
use crate::slots::{Host, Slots};

impl Shell {
    /// Opens the view of the [`RailView`](crate::slots::RailView) registered as `id`. Nothing happens for an id nobody
    /// registered, so a module that is not in the app leaves its doors dead and nothing else.
    pub fn open_view(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(view) = cx.global::<Slots>().view(id).cloned() else {
            return;
        };
        // The Usage view is a lens of the shell, not a layer over it: the next frame shows it.
        if id == "usage" {
            self.usage_asked = true;
            cx.notify();
            return;
        }
        let host = Host::new(cx.weak_entity(), self.vitals.clone());
        let page = (view.open)(&host, cx);
        self.opened = Some((view.id, page));
        cx.notify();
    }

    pub fn close_view(&mut self, cx: &mut Context<Self>) {
        self.opened = None;
        cx.notify();
    }
}
