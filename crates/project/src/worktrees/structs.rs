use std::path::PathBuf;

/// One worktree of a project's repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    /// Its folder, as the host names it.
    pub path: PathBuf,
    /// Its branch, without `refs/heads/`; `None` when its HEAD is detached.
    pub branch: Option<String>,
    /// The commit it has checked out; `None` before the first commit.
    pub head: Option<String>,
    /// The repository's main checkout, listed first.
    pub main: bool,
    /// Why it is locked (`""` when no reason was given); `None` when it is not.
    pub locked: Option<String>,
    /// Why git would prune it, such as a folder that is gone; `None` when it would not.
    pub prunable: Option<String>,
    /// What it holds; `None` when that cannot be read, as when its folder is gone.
    pub state: Option<TreeState>,
}

/// What a worktree holds that its repository may not.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TreeState {
    /// Tracked files changed and not staged.
    pub changed: usize,
    pub staged: usize,
    pub untracked: usize,
    /// Files with a conflict not yet resolved.
    pub conflicted: usize,
    pub upstream: Upstream,
    /// Commits of its HEAD on no other branch, local or remote: what removing it and its branch would lose.
    pub only_here: usize,
    /// Whether merging it into the default branch would change nothing, so it was merged, squashed or not.
    /// `None` when the repository has no default branch to ask.
    pub merged: Option<bool>,
}

/// The remote branch a worktree's branch follows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Upstream {
    /// It follows none.
    #[default]
    None,
    Tracking { ahead: usize, behind: usize },
    /// It followed one that is gone, as after a merged pull request's branch is deleted.
    Gone,
}

impl Worktree {
    /// Removing it loses nothing: no uncommitted file, and every commit of it merged or on another branch.
    pub fn nothing_to_lose(&self) -> bool {
        self.state.as_ref().is_some_and(|s| {
            s.changed + s.staged + s.untracked + s.conflicted == 0 && (s.only_here == 0 || s.merged == Some(true))
        })
    }
}
