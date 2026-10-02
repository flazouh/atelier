use std::rc::Rc;

use gpui_kit::{App, Window};

/// The widest a session's rows and composer grow, in design pixels: a session alone in a wide window keeps a
/// reading width, centred, and the margin either side stays the panel's.
pub const READING_WIDTH: f32 = 768.;

/// How much room an item takes in the list: a flat row (reading, searching), a card, or prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Block {
    Flat,
    Card,
    Prose,
}

/// What a choice in the panel's menu does.
pub(super) type MenuAction = Rc<dyn Fn(&mut Window, &mut App)>;
