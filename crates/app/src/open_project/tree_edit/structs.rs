use gpui_kit::{Entity, Pixels, Point, Subscription, component::input::InputState};

use super::types::TreeEditKind;

/// The menu open on a row of the tree, or on its empty part.
#[derive(Clone, Debug)]
pub struct TreeMenu {
    /// The row's path; `""` for the tree's empty part, which is the project's folder.
    pub path: String,
    pub dir: bool,
    /// Where the press was, in the window.
    pub at: Point<Pixels>,
}

/// A name being typed in the tree.
pub struct TreeEdit {
    pub kind: TreeEditKind,
    pub input: Entity<InputState>,
    pub(super) _events: Subscription,
}
