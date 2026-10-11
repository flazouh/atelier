use std::rc::Rc;

use gpui_kit::{App, Window};

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

    /// Starts a session that belongs to the bot kept under the id `bot`, in the project in front, and brings the
    /// Sessions view in front with it open. Call it from a handler, which has the window. The app says why when none
    /// starts: no project is open, or the bot is gone.
    pub fn start_session_as(&self, bot: &str, window: &mut Window, cx: &mut App) {
        self.0.start_session_as(bot, window, cx);
    }
}
