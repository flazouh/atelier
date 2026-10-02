//! A project's worktrees as the Git view lists them: each `atelier_project::Worktree` in plain words, warnings
//! for what removing it would lose, and a good word when it is merged.
use std::path::Path;

use atelier_project::{Upstream, Worktree};
use atelier_ui::worktree_list::{NoteTone, WorktreeNote, WorktreeRow};

/// The rows for `trees`, in their order. A folder under `home` is shown from `~`; `sessions` counts the sessions
/// working in a folder.
pub fn rows(trees: &[Worktree], home: Option<&Path>, sessions: impl Fn(&Path) -> usize) -> Vec<WorktreeRow> {
    trees
        .iter()
        .map(|tree| WorktreeRow {
            path: tree.path.to_string_lossy().into_owned().into(),
            folder: shown(&tree.path, home).into(),
            branch: tree.branch.clone().map(Into::into),
            main: tree.main,
            notes: notes(tree),
            sessions: sessions(&tree.path),
        })
        .collect()
}

fn shown(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// What `tree` holds, worst first.
pub fn notes(tree: &Worktree) -> Vec<WorktreeNote> {
    use NoteTone::{Good, Quiet, Warning};
    let mut notes = Vec::new();
    if tree.prunable.is_some() {
        notes.push(WorktreeNote::new("folder missing", Warning));
    }
    if tree.locked.is_some() {
        notes.push(WorktreeNote::new("locked", Quiet));
    }
    let Some(state) = &tree.state else {
        if tree.prunable.is_none() {
            notes.push(WorktreeNote::new("could not be read", Warning));
        }
        return notes;
    };
    if state.conflicted > 0 {
        notes.push(WorktreeNote::new(format!("{} conflicted", state.conflicted), Warning));
    }
    let uncommitted = state.changed + state.staged + state.untracked;
    if uncommitted > 0 {
        notes.push(WorktreeNote::new(format!("{uncommitted} uncommitted"), Warning));
    }
    let merged = state.merged == Some(true);
    if state.only_here > 0 && !merged {
        notes.push(WorktreeNote::new(format!("{} only here", state.only_here), Warning));
    }
    match state.upstream {
        Upstream::Tracking { behind, .. } if behind > 0 => notes.push(WorktreeNote::new(format!("{behind} behind"), Quiet)),
        Upstream::Gone if !merged => notes.push(WorktreeNote::new("remote branch gone", Quiet)),
        _ => {}
    }
    if merged && !tree.main {
        notes.push(WorktreeNote::new("merged", Good));
    }
    notes
}

#[cfg(test)]
mod tests;
