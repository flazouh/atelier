use std::rc::Rc;

use gpui_kit::{AnyElement, App, Entity};

use crate::traits::{AnyPage, PluginPage};

/// The page of a view, of any type, as the app keeps it. The app draws its two parts and tells it when its view comes
/// in front again.
#[derive(Clone)]
pub struct Page(Rc<dyn AnyPage>);

impl Page {
    pub(crate) fn new<P: PluginPage>(page: Entity<P>) -> Self {
        Self(Rc::new(page))
    }

    pub fn sidebar(&self, cx: &mut App) -> AnyElement {
        self.0.sidebar(cx)
    }

    pub fn main(&self, cx: &mut App) -> AnyElement {
        self.0.main(cx)
    }

    pub fn in_front_again(&self, cx: &mut App) {
        self.0.in_front_again(cx);
    }

    /// The page as its own type, for the code that knows which plugin made it; none for another type.
    pub fn downcast<P: PluginPage>(&self) -> Option<Entity<P>> {
        self.0.entity().downcast().ok()
    }
}
