//! How the window's width divides between the sidebar, the session column and the right pane. The
//! rules, with their reasons, are in docs/app.md under "Window widths".

/// Below this width the window shows one pane at a time, with tabs.
pub const NARROW_BELOW: f32 = 900.;
/// From this width the sidebar shows beside the two panes; below it, only on ⌘B.
pub const WIDE_FROM: f32 = 1100.;
pub const SIDEBAR_LEAST: f32 = 180.;
pub const SIDEBAR_MOST: f32 = 480.;
pub const SIDEBAR_DEFAULT: f32 = 260.;
/// The session column's least width: a session panel's least width, which it never goes under.
pub const AGENT_LEAST: f32 = atelier_ui::panel_layout::MIN_WIDTH;
pub const RIGHT_LEAST: f32 = 320.;
pub const RIGHT_MOST: f32 = 2400.;
pub const RIGHT_DEFAULT: f32 = 560.;

/// The layout a window's width gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// One pane at a time, with tabs.
    Narrow,
    /// The session column and the right pane; the sidebar on ⌘B.
    Medium,
    /// The sidebar, the session column and the right pane.
    Wide,
}

impl Fit {
    pub fn of(width: f32) -> Self {
        if width < NARROW_BELOW {
            Fit::Narrow
        } else if width < WIDE_FROM {
            Fit::Medium
        } else {
            Fit::Wide
        }
    }
}

/// The pane a narrow window shows, one at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    /// The sidebar: the projects, their sessions and the files.
    Projects,
    Session,
    /// The editor, or the review or the pull requests in its place.
    Right,
}

/// The side panes' widths the reader asked for; `None` for a pane that is hidden.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wants {
    pub sidebar: Option<f32>,
    pub right: Option<f32>,
}

/// The widths the panes get; `None` for a pane that does not show.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Widths {
    pub sidebar: Option<f32>,
    pub agent: f32,
    pub right: Option<f32>,
}

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

#[cfg(test)]
mod tests;
