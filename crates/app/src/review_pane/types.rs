use std::{
    time::{Duration},
};

use atelier_ui::RowMap;
use gpui_kit::{Entity, SharedString, component::input::EditorState};
use atelier_editor::EditorSession;
use atelier_project::Project;

/// Makes the language server session for a file of the project, shown with the given rows over it.
pub type SessionFor = std::rc::Rc<dyn Fn(&str, Entity<EditorState>, RowMap, &mut gpui_kit::App) -> Entity<EditorSession>>;

/// How long after the reader's last key the file is written.
pub(super) const WRITE_AFTER: Duration = Duration::from_millis(300);

/// Below this width the tree hides, and `s`, `w` and the bar walk the files; review mode shows it.
pub(super) const TREE_FROM: f32 = 680.;

/// Which changes the review holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    /// One turn, by its index in the session.
    Turn(usize),
    /// The session as one change.
    Whole,
}

impl Scope {
    /// The key `Reviewed` keeps marks under: the turn, or a key no turn has for the whole session.
    pub fn key(self) -> usize {
        match self {
            Self::Turn(turn) => turn,
            Self::Whole => usize::MAX,
        }
    }
}

/// A change the pane makes on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum DiskChange {
    Write(String),
    Remove,
}

impl DiskChange {
    /// The file on disk once this is done.
    pub(super) fn on_disk(&self) -> Option<String> {
        match self {
            Self::Write(text) => Some(text.clone()),
            Self::Remove => None,
        }
    }

    pub(super) fn apply(&self, project: &dyn Project, path: &str) -> std::io::Result<()> {
        match self {
            Self::Write(text) => project.write(path, text.as_bytes()),
            Self::Remove => project.remove(path),
        }
    }
}

pub enum PaneEvent {
    /// Escape or the bar's close: the pane goes, and the editor comes back.
    Close,
    /// A line for the status line, such as a write that failed.
    Said(SharedString),
    /// The strip made a commit or a branch, so the branch and its changes are to be read again.
    GitChanged,
    /// The reader asked to see this pull request.
    ShowPull(atelier_forge::PullRef),
}
