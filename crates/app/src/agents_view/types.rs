use gpui_kit::SharedString;
use atelier_agents::session::SessionId;

pub(super) const PAST: &str = "past:";

/// What a sidebar row or a panel names.
pub enum Pick {
    /// A session open in the window, by its key.
    Open(SharedString),
    /// A past session, to resume.
    Past(SessionId),
}
