use std::rc::Rc;

use gpui_kit::{App, Window};

/// How much room an item takes in the list: a flat row (reading, searching), a card, or prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Block {
    Flat,
    Card,
    Prose,
}

/// What a choice in the panel's menu does.
pub(super) type MenuAction = Rc<dyn Fn(&mut Window, &mut App)>;
