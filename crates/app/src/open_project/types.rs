use gpui_kit::{Entity, SharedString};
use atelier_agents::session::SessionId;

use crate::{agent_session::AgentSession, tree::ProjectTree};

/// What `git` says about the folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Git {
    /// Not asked yet.
    Unknown,
    /// The folder is not in a git repository.
    None,
    Branch(SharedString),
    /// A repository with no commit yet, on this branch.
    Unborn(SharedString),
}

impl Git {
    /// The branch, born or not.
    pub fn branch(&self) -> Option<&SharedString> {
        match self {
            Git::Branch(b) | Git::Unborn(b) => Some(b),
            Git::Unknown | Git::None => None,
        }
    }
}

/// The tree as the last listing left it.
#[derive(Clone, Debug)]
pub enum Listing {
    Loading,
    Ready(ProjectTree),
    Failed(SharedString),
}

/// Whether a tab's file is still on disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deleted {
    No,
    /// Deleted on disk; the tab asks: Close or Keep.
    Asking,
    /// The reader kept the text; a save asks before it creates the file again.
    Kept,
}

pub enum ProjectEvent {
    /// A line for the status line, such as a save that failed.
    Said(SharedString),
    /// A session started, changed or ended, or the past list came in: the sidebar draws again.
    Sessions,
    /// The reader named a session.
    Renamed { id: SessionId, name: SharedString },
    /// The reader asked to review a session's turn (`None`: the whole session), at a file.
    Review { session: Entity<AgentSession>, turn: Option<usize>, path: Option<String> },
    /// The reader asked to open a file, by its path in the project.
    Open(String),
    /// The pull requests came on screen: the shell shows the right pane.
    PullsShown,
    /// A session the project opened for a task: the shell puts it in front.
    ShowSession(Entity<AgentSession>),
    /// The tasks came on screen: the shell shows the right pane.
    TasksShown,
    /// The review closed: the editor is back.
    ReviewClosed,
    /// The reader pressed a session panel's close button, by the session's key.
    CloseSession(SharedString),
    /// The reader asked in a panel's menu for a new session in the panel's project.
    NewSessionHere,
    /// The reader asked in a panel's menu to archive the session, by its key.
    ArchiveSession(SharedString),
    /// The reader ran `/files` in a session: the Files view comes to the front.
    ShowFiles,
    /// The reader ran `/tasks` in a session: the tasks come to the right pane.
    ShowTasks,
}

/// Why a project's pull requests do not open.
pub(crate) const NO_FORGE_REMOTE: &str = "No GitHub remote for this project";

#[cfg(test)]
thread_local! {
    /// Set by a test that drives a whole shell: the disk watch's thread and a real agent's would wake it off its clock,
    /// so there is no watch and its sessions talk to a fake agent on the test's thread.
    pub(crate) static TEST_THREAD_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
