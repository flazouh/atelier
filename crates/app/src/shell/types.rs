use std::sync::Arc;

/// The title bar's height, and the room the macOS window buttons take at its left.
pub const TITLE_BAR: f32 = 38.;

/// How long a notice stays at the foot of the window.
pub(super) const NOTICE_FOR: std::time::Duration = std::time::Duration::from_secs(5);

pub(super) const TRAFFIC_LIGHTS: f32 = if cfg!(target_os = "macos") { 78. } else { 12. };

/// How often the sidebar's ages are brought up to date.
pub(super) const AGE_TICK: std::time::Duration = std::time::Duration::from_secs(60);

/// The right pane's width the tasks and the pull requests open at: less when the window has not got
/// it, since the agent panel keeps its least width.
pub(super) const WIDE_RIGHT: f32 = 860.;

/// What a wide right pane leaves the agent panel: a session panel at its default width, and its margins.
pub(super) const AGENT_BESIDE_RIGHT: f32 = atelier_ui::panel_layout::DEFAULT_WIDTH + 2. * atelier_ui::panel_layout::GAP + 4.;

/// What the first launch says atelier is, in one line.
pub(super) const WHAT_ATELIER_IS: &str = "Run coding agents on your code, review every change they make, and commit what you keep.";

/// Whose folders the folder picker lists.
#[derive(Clone)]
pub(super) enum FolderSource {
    Local,
    /// A host's, through a project connected at its home folder.
    Remote { host: String, project: Arc<dyn atelier_project::Project> },
}

/// A pane edge a reader drags to size the pane beside it.
#[derive(Clone, Copy)]
pub(super) enum Edge {
    Sidebar,
    Right,
}
