use gpui_kit::{Entity, Subscription};
use atelier_pr_view::hub::PrHub;

pub struct Pulls {
    pub hub: Entity<PrHub>,
    /// In the right pane now; the hub keeps its state while hidden.
    pub shown: bool,
    /// The hub's events, the list's opens, and the list feeding the chips.
    pub _events: [Subscription; 3],
}
