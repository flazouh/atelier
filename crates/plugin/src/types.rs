use std::rc::Rc;

use gpui_kit::App;

use super::structs::{Host, Page};

/// Makes the page of a [`PluginView`](super::PluginView), with the host it may use.
pub(super) type OpenPage = Rc<dyn Fn(&Host, &mut App) -> Page>;
