use std::path::{Path, PathBuf};

use atelier_project::{TreeState, Upstream, Worktree};
use atelier_ui::worktree_list::NoteTone::{Good, Quiet, Warning};

use super::*;

fn tree(path: &str, state: Option<TreeState>) -> Worktree {
    Worktree { path: PathBuf::from(path), branch: Some("fix".into()), head: Some("a".repeat(40)), main: false, locked: None, prunable: None, state }
}

fn said(tree: &Worktree) -> Vec<(String, atelier_ui::worktree_list::NoteTone)> {
    notes(tree).into_iter().map(|n| (n.words.to_string(), n.tone)).collect()
}

#[test]
fn a_worktree_with_work_in_it_warns_of_what_removing_it_would_lose() {
    let state = TreeState { changed: 1, staged: 1, untracked: 1, conflicted: 1, only_here: 2, merged: Some(false), upstream: Upstream::Tracking { ahead: 2, behind: 4 } };
    assert_eq!(
        said(&tree("/r/fix", Some(state))),
        [("1 conflicted".into(), Warning), ("3 uncommitted".into(), Warning), ("2 only here".into(), Warning), ("4 behind".into(), Quiet)]
    );
}

#[test]
fn a_merged_worktree_says_so_and_its_own_commits_are_no_warning() {
    let state = TreeState { only_here: 3, merged: Some(true), upstream: Upstream::Gone, ..TreeState::default() };
    assert_eq!(said(&tree("/r/fix", Some(state.clone()))), [("merged".into(), Good)]);
    let main = Worktree { main: true, ..tree("/r/main", Some(TreeState { merged: Some(true), ..TreeState::default() })) };
    assert_eq!(said(&main), [], "the main checkout is not called merged");
    let gone = TreeState { only_here: 1, merged: Some(false), upstream: Upstream::Gone, ..TreeState::default() };
    assert_eq!(said(&tree("/r/fix", Some(gone))), [("1 only here".into(), Warning), ("remote branch gone".into(), Quiet)]);
}

#[test]
fn a_worktree_that_cannot_be_read_is_a_warning_never_a_clean_row() {
    let missing = Worktree { prunable: Some("gitdir file points to non-existent location".into()), ..tree("/r/gone", None) };
    assert_eq!(said(&missing), [("folder missing".into(), Warning)]);
    assert_eq!(said(&tree("/r/odd", None)), [("could not be read".into(), Warning)]);
    let locked = Worktree { locked: Some(String::new()), ..tree("/r/usb", Some(TreeState::default())) };
    assert_eq!(said(&locked), [("locked".into(), Quiet)]);
}

#[test]
fn rows_show_folders_from_home_and_count_each_ones_sessions() {
    let trees = [Worktree { main: true, ..tree("/home/a/code/atelier", Some(TreeState::default())) }, tree("/srv/fix", Some(TreeState::default()))];
    let rows = rows(&trees, Some(Path::new("/home/a")), |p| usize::from(p == Path::new("/home/a/code/atelier")) * 2);
    let seen: Vec<(&str, &str, bool, usize)> = rows.iter().map(|r| (r.path.as_ref(), r.folder.as_ref(), r.main, r.sessions)).collect();
    assert_eq!(seen, [("/home/a/code/atelier", "~/code/atelier", true, 2), ("/srv/fix", "/srv/fix", false, 0)]);
    assert_eq!(shown(Path::new("/home/a"), Some(Path::new("/home/a"))), "~");
}
