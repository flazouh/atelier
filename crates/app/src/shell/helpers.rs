use gpui_kit::{App, KeyBinding};

use super::structs::{
    CloseTab, NewSession, OpenFolder, OpenRemote, OpenSettings, OpenTasks, PullRequests, Quit,
    Save, ShowSessions, ToggleRight, ToggleSidebar, ZoomIn, ZoomOut, ZoomReset,
};

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
