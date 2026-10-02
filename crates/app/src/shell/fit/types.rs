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
