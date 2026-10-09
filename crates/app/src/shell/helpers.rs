use gpui_kit::{App, KeyBinding};

use super::structs::{
    CloseTab, NewSession, OpenFolder, OpenRemote, OpenSettings, OpenTasks, PullRequests, Quit,
    Save, ShowSessions, ToggleRight, ToggleSidebar, ZoomIn, ZoomOut, ZoomReset,
};

/// How wide the changelog sheet is in a window `viewport` design pixels wide: its own width, or the window's less a margin at
/// each side when that is narrower, and never narrower than it can read.
pub(super) fn sheet_width(viewport: f32) -> f32 {
    (viewport - 2. * super::types::SHEET_MARGIN).clamp(super::types::SHEET_MIN, super::types::SHEET_WIDTH)
}

/// Where the shell keeps what it saves. A test writes only where `ATELIER_SETTINGS` points, never the
/// reader's own settings.
pub(super) fn settings_path() -> Option<std::path::PathBuf> {
    if cfg!(test) && std::env::var_os("ATELIER_SETTINGS").is_none() {
        return None;
    }
    atelier_settings::path()
}

pub fn bind_keys(cx: &mut App) {
    crate::ship::strip::bind_keys(cx);
    crate::ship::pull_form::bind_keys(cx);
    cx.bind_keys([
        KeyBinding::new("secondary-o", OpenFolder, None),
        KeyBinding::new("secondary-1", ShowSessions, None),
        // Zoom, as in every editor: ⌘+ (and ⌘=, the same key unshifted), ⌘−, ⌘0.
        KeyBinding::new("secondary-=", ZoomIn, None),
        KeyBinding::new("secondary-+", ZoomIn, None),
        KeyBinding::new("secondary--", ZoomOut, None),
        KeyBinding::new("secondary-0", ZoomReset, None),
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("secondary-shift-o", OpenRemote, None),
        KeyBinding::new("secondary-shift-O", OpenRemote, None),
        KeyBinding::new("secondary-s", Save, None),
        KeyBinding::new("secondary-n", NewSession, None),
        KeyBinding::new("secondary-w", CloseTab, None),
        KeyBinding::new("secondary-b", ToggleSidebar, None),
        // A shifted combo arrives with the letter either way, depending on the platform.
        KeyBinding::new("secondary-shift-b", ToggleRight, None),
        KeyBinding::new("secondary-shift-B", ToggleRight, None),
        KeyBinding::new("secondary-,", OpenSettings, None),
        KeyBinding::new("secondary-shift-p", PullRequests, None),
        KeyBinding::new("secondary-shift-l", OpenTasks, None),
        KeyBinding::new("secondary-shift-L", OpenTasks, None),
        KeyBinding::new("secondary-shift-P", PullRequests, None),
    ]);
}

/// A failure to read or open a folder, as the picker tells it.
pub(super) fn folder_error(error: &std::io::Error) -> atelier_ui::FolderError {
    match error.kind() {
        std::io::ErrorKind::NotFound => atelier_ui::FolderError::Missing,
        std::io::ErrorKind::PermissionDenied => atelier_ui::FolderError::Denied,
        std::io::ErrorKind::NotADirectory => atelier_ui::FolderError::NotAFolder,
        _ => atelier_ui::FolderError::Other(error.to_string().into()),
    }
}

/// How a project's sessions stand, for the dot on its badge in the switcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProjectMark {
    /// A session waits on the reader: an approval or a question.
    NeedsYou,
    /// A session is at work.
    Working,
    Quiet,
}

/// The mark of a project whose sessions stand at `statuses`: one that needs the reader outweighs one at work.
pub(super) fn project_mark(statuses: &[atelier_ui::session_status::SessionStatus]) -> ProjectMark {
    if statuses.iter().any(|s| s.needs_you()) {
        ProjectMark::NeedsYou
    } else if statuses.iter().any(|s| matches!(s, atelier_ui::session_status::SessionStatus::Working)) {
        ProjectMark::Working
    } else {
        ProjectMark::Quiet
    }
}

/// The count on the Changes row: what the checkout holds uncommitted once git has been read, else what the front session
/// changed. The row opens the checkout's changes, so its count is theirs.
pub(super) fn changes_badge(uncommitted: Option<&crate::history::Read<Vec<crate::history::CommitFile>>>, in_session: usize) -> Option<usize> {
    let count = match uncommitted {
        Some(crate::history::Read::Ready(files)) => files.len(),
        _ => in_session,
    };
    (count > 0).then_some(count)
}

/// How much of the title bar's free room, at its right, the session tabs leave to the ⋯ of the layout menu: the ⋯ stands
/// at the right of the session area, or left of the Settings button when that area reaches the window's edge.
pub(super) fn tab_room(width: f32, session_right: Option<f32>, chip: f32) -> f32 {
    // The room ends 48 px short of the window's edge (the Settings button, its gap and the bar's padding); the ⋯ is 28 wide.
    let region_right = width - 48. - chip;
    let more_left = match session_right {
        Some(right) => (right - 36.).min(width - 72. - chip),
        None => width - 48. - chip - 28.,
    };
    (region_right - (more_left - 8.)).max(0.)
}
