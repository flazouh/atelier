use std::rc::Rc;

use atelier_ui::IconName;
use gpui_kit::{App, AppContext, Context, SharedString};

use super::{Host, Page};
use crate::{traits::PluginPage, types::OpenPage};

/// A view a plugin adds: its entry on the left rail, and the page the entry opens.
#[derive(Clone)]
pub struct PluginView {
    /// Names the view: the settings keep the view in front by it, and [`Host::open_view`] opens it by it. A view
    /// registered with the id of another takes its place.
    pub id: &'static str,
    /// The icon of the entry on the rail.
    pub icon: IconName,
    /// The name of the entry.
    pub label: SharedString,
    /// Entries stand from the lowest order to the highest, after the app's own; the same order goes by id.
    pub order: i32,
    open: OpenPage,
}

impl PluginView {
    /// A view whose page `page` makes. The app calls `page` once, the first time the view is in front, and keeps what
    /// it made.
    pub fn new<P: PluginPage>(
        id: &'static str,
        icon: IconName,
        label: impl Into<SharedString>,
        order: i32,
        page: impl Fn(&Host, &mut Context<P>) -> P + 'static,
    ) -> Self {
        Self {
            id,
            icon,
            label: label.into(),
            order,
            open: Rc::new(move |host, cx| Page::new(cx.new(|cx| page(host, cx)))),
        }
    }

    /// Makes the page. The app calls this, with its host.
    pub fn open(&self, host: &Host, cx: &mut App) -> Page {
        (self.open)(host, cx)
    }
}
