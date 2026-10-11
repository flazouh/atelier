use std::rc::Rc;

use gpui_kit::App;

use super::Vitals;
use crate::traits::AppHost;

/// The handle a plugin gets to the app: the few things it may ask the app to do or to tell. A clone is cheap, so a page
/// keeps its own and gives one to each handler that needs it.
#[derive(Clone)]
pub struct Host(Rc<dyn AppHost>);

impl Host {
    /// The host of `app`. Only the app, or a test that stands for it, makes one.
    pub fn new(app: impl AppHost + 'static) -> Self {
        Self(Rc::new(app))
    }

    /// Brings the registered view `id` in front, as a press on its entry does. Nothing happens for an id nobody
    /// registered.
    pub fn open_view(&self, id: &str, cx: &mut App) {
        self.0.open_view(id, cx);
    }

    /// The numbers of the status bar, as the app last read them.
    pub fn vitals(&self, cx: &App) -> Vitals {
        self.0.vitals(cx)
    }
}
