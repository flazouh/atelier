use super::structs::{Wants, Widths};
use super::types::{AGENT_LEAST, RIGHT_LEAST, RIGHT_MOST, SIDEBAR_LEAST, SIDEBAR_MOST};

/// Divides `total` so the session column keeps its least width. The right pane gives way first, down
/// to its least width, then the sidebar; a pane that cannot keep its least width hides, so nothing
/// clips or draws under its neighbour.
pub fn widths(total: f32, wants: Wants) -> Widths {
    let mut sidebar = wants.sidebar.map(|s| s.clamp(SIDEBAR_LEAST, SIDEBAR_MOST));
    let mut right = wants.right.map(|r| r.clamp(RIGHT_LEAST, RIGHT_MOST));
    let short = |sidebar: Option<f32>, right: Option<f32>| AGENT_LEAST - (total - sidebar.unwrap_or(0.) - right.unwrap_or(0.));
    let over = short(sidebar, right);
    if over > 0. {
        right = right.map(|r| (r - over).max(RIGHT_LEAST));
    }
    let over = short(sidebar, right);
    if over > 0. {
        sidebar = sidebar.map(|s| s - over).filter(|s| *s >= SIDEBAR_LEAST);
    }
    if short(sidebar, right) > 0. {
        right = None;
    }
    let agent = total - sidebar.unwrap_or(0.) - right.unwrap_or(0.);
    Widths { sidebar, agent, right }
}
