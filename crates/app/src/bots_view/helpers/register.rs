use atelier_ui::IconName;
use gpui_kit::AppContext;

use crate::slots::{RailView, Slots};

use super::super::structs::BotsPage;
use super::library_root;

/// What this module adds to the app: the Bots view, which the rail and a module's `open_view("bots")` open. With the
/// registration gone, a view named bots opens nothing.
pub fn register(slots: &mut Slots) {
    // The shell shows this lens itself and gives the page its root; a page opened this way reads the reader's folder.
    slots.add_view(RailView::new("bots", Some(IconName::Bot), "Bots", 90, |_, cx| {
        cx.new(|cx| BotsPage::new(library_root(atelier_settings::path().as_deref()), cx)).into()
    }));
}
