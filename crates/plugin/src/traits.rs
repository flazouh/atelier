use gpui_kit::{AnyElement, AnyEntity, App, Context, Entity};

use super::structs::{Registry, Vitals};

/// One plugin. Its one job is to register what it adds to the app. The app calls it once, at startup, before the
/// window opens.
pub trait Plugin {
    fn register(&self, registry: &mut Registry);
}

/// What a view shows. The app keeps one page for each view, made the first time the view is in front, and asks it for
/// the two parts of the window a view fills.
pub trait PluginPage: Sized + 'static {
    /// The sidebar of the view. The app puts it under the project switcher, as it does for its own views.
    fn sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement;

    /// The main area of the view. The app puts it on a card beside the sidebar; the page scrolls inside the card.
    fn main(&mut self, cx: &mut Context<Self>) -> AnyElement;

    /// The view came in front again, after another one. A page that reads something (a folder, a log) reads it again
    /// here. The app does not call it the first time: the page is made then, and reads as it is made.
    fn in_front_again(&mut self, _cx: &mut Context<Self>) {}
}

/// What the app does for a plugin. The app implements it, and a plugin calls it through [`Host`](super::Host).
pub trait AppHost {
    /// Brings the registered view `id` in front. Nothing happens for an id nobody registered.
    fn open_view(&self, id: &str, cx: &mut App);

    /// The numbers of the status bar, as the app last read them.
    fn vitals(&self, cx: &App) -> Vitals;
}

/// A page of any type, as the app calls it: [`Page`](super::Page) holds one.
pub(super) trait AnyPage {
    fn sidebar(&self, cx: &mut App) -> AnyElement;
    fn main(&self, cx: &mut App) -> AnyElement;
    fn in_front_again(&self, cx: &mut App);
    fn entity(&self) -> AnyEntity;
}

impl<P: PluginPage> AnyPage for Entity<P> {
    fn sidebar(&self, cx: &mut App) -> AnyElement {
        self.update(cx, |page, cx| page.sidebar(cx))
    }

    fn main(&self, cx: &mut App) -> AnyElement {
        self.update(cx, |page, cx| page.main(cx))
    }

    fn in_front_again(&self, cx: &mut App) {
        self.update(cx, |page, cx| page.in_front_again(cx));
    }

    fn entity(&self) -> AnyEntity {
        self.clone().into_any()
    }
}
