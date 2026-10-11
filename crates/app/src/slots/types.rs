use std::rc::Rc;

use gpui_kit::{AnyElement, App, Window};

use super::structs::BarEnv;
use crate::vitals::Vitals;

/// The id of the status bar: its parts take their own ids from it.
pub const BAR_ID: &str = "status-bar";

/// The three columns of the bar, as the panes above it stand. Under a hidden sidebar the left column's cards lead the middle
/// one; with no right pane, or nothing in the right column, the right column's cards end the middle one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    /// Under the sidebar.
    Left,
    Middle,
    /// Under the right pane.
    Right,
}

/// Whether a card is drawn now.
pub type Visible = Rc<dyn Fn(&Vitals) -> bool>;

/// What a card draws: the parts it puts in its column, left to right.
pub type RenderCard = Rc<dyn Fn(&BarEnv<'_>, &mut Window, &mut App) -> Vec<AnyElement>>;
