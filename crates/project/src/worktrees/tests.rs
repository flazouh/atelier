use std::{
    fs,
    path::{Path, PathBuf},
};

use super::*;
use crate::LocalProject;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn commit(dir: &Path, file: &str, text: &str) {
    fs::write(dir.join(file), text).unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-qm", file]);
}

/// A clone of a bare `origin` with one commit on `main`, its folders canonical.
struct Repo {
    dir: tempfile::TempDir,
    main: PathBuf,
}

impl Repo {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let origin = dir.path().join("origin.git");
        git(dir.path(), &["init", "-q", "--bare", "-b", "main", origin.to_str().unwrap()]);
        let main = dir.path().join("main");
        git(dir.path(), &["clone", "-q", origin.to_str().unwrap(), main.to_str().unwrap()]);
        git(&main, &["checkout", "-q", "-b", "main"]);
        commit(&main, "a.txt", "a");
        git(&main, &["push", "-q", "-u", "origin", "main"]);
        git(&main, &["remote", "set-head", "origin", "main"]);
        let main = fs::canonicalize(main).unwrap();
        Self { dir, main }
    }

    /// A worktree on a new branch `name`, beside the main checkout.
    fn tree(&self, name: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        git(&self.main, &["worktree", "add", "-q", "-b", name, path.to_str().unwrap()]);
        fs::canonicalize(path).unwrap()
    }

    fn read(&self) -> Vec<Worktree> {
        worktrees(&LocalProject::open(&self.main).unwrap()).unwrap()
    }

    fn find(&self, path: &Path) -> Worktree {
        self.read().into_iter().find(|w| w.path == path).expect("listed")
    }
}

#[test]
fn the_main_checkout_comes_first_then_each_worktree_with_its_branch() {
    let repo = Repo::new();
    let fix = repo.tree("fix");
    let loose = repo.dir.path().join("loose");
    git(&repo.main, &["worktree", "add", "-q", "--detach", loose.to_str().unwrap()]);
    let all = repo.read();
    let seen: Vec<(PathBuf, Option<&str>, bool)> = all.iter().map(|w| (w.path.clone(), w.branch.as_deref(), w.main)).collect();
    assert_eq!(
        seen,
        [(repo.main.clone(), Some("main"), true), (fix, Some("fix"), false), (fs::canonicalize(&loose).unwrap(), None, false)]
    );
    assert!(all.iter().all(|w| w.head.as_ref().is_some_and(|h| h.len() == 40)), "{all:?}");
}

#[test]
fn a_clean_worktree_with_nothing_new_has_nothing_to_lose() {
    let repo = Repo::new();
    let tree = repo.find(&repo.tree("fix"));
    let state = tree.state.clone().expect("its folder is there");
    assert_eq!((state.changed, state.staged, state.untracked, state.conflicted), (0, 0, 0, 0));
    assert_eq!(state.upstream, Upstream::None);
    assert_eq!(state.only_here, 0);
    assert_eq!(state.merged, Some(true));
    assert!(tree.nothing_to_lose());
}

#[test]
fn uncommitted_files_are_counted_by_kind() {
    let repo = Repo::new();
    let path = repo.tree("fix");
    fs::write(path.join("a.txt"), "changed").unwrap();
    fs::write(path.join("b.txt"), "staged").unwrap();
    git(&path, &["add", "b.txt"]);
    fs::create_dir(path.join("new")).unwrap();
    fs::write(path.join("new/c.txt"), "untracked").unwrap();
    fs::write(path.join("new/d.txt"), "untracked").unwrap();
    let tree = repo.find(&path);
    let state = tree.state.clone().unwrap();
    assert_eq!((state.changed, state.staged, state.untracked), (1, 1, 2));
    assert!(!tree.nothing_to_lose());
}

#[test]
fn commits_found_nowhere_else_are_counted_with_or_without_an_upstream() {
    let repo = Repo::new();
    let path = repo.tree("fix");
    commit(&path, "b.txt", "b");
    commit(&path, "c.txt", "c");
    let lone = repo.find(&path).state.unwrap();
    assert_eq!((lone.upstream.clone(), lone.only_here, lone.merged), (Upstream::None, 2, Some(false)));

    git(&path, &["push", "-q", "-u", "origin", "fix"]);
    commit(&path, "d.txt", "d");
    let pushed = repo.find(&path);
    let state = pushed.state.clone().unwrap();
    assert_eq!(state.upstream, Upstream::Tracking { ahead: 1, behind: 0 });
    assert_eq!(state.only_here, 1, "the pushed two are on the remote");
    assert!(!pushed.nothing_to_lose());
}

#[test]
fn a_squash_merged_branch_whose_remote_branch_is_gone_counts_as_merged() {
    let repo = Repo::new();
    let path = repo.tree("fix");
    commit(&path, "b.txt", "b");
    commit(&path, "c.txt", "c");
    git(&path, &["push", "-q", "-u", "origin", "fix"]);
    git(&repo.main, &["merge", "-q", "--squash", "fix"]);
    git(&repo.main, &["commit", "-qm", "fix (#1)"]);
    git(&repo.main, &["push", "-q", "origin", "main"]);
    git(&repo.main, &["push", "-q", "origin", "--delete", "fix"]);
    git(&repo.main, &["fetch", "-q", "--prune"]);
    let tree = repo.find(&path);
    let state = tree.state.clone().unwrap();
    assert_eq!(state.upstream, Upstream::Gone);
    assert_eq!(state.only_here, 2, "its own commits are on no other branch");
    assert_eq!(state.merged, Some(true), "merging it again would change nothing");
    assert!(tree.nothing_to_lose());
}

#[test]
fn a_worktree_whose_folder_is_gone_is_prunable_and_has_no_state() {
    let repo = Repo::new();
    let path = repo.tree("fix");
    fs::remove_dir_all(&path).unwrap();
    let tree = repo.read().into_iter().find(|w| w.branch.as_deref() == Some("fix")).unwrap();
    assert!(tree.prunable.is_some(), "{tree:?}");
    assert_eq!(tree.state, None);
    assert!(!tree.nothing_to_lose(), "what was there cannot be checked");
}

#[test]
fn a_locked_worktree_says_why() {
    let repo = Repo::new();
    let path = repo.tree("fix");
    git(&repo.main, &["worktree", "lock", "--reason", "on a usb disk", path.to_str().unwrap()]);
    assert_eq!(repo.find(&path).locked.as_deref(), Some("on a usb disk"));
}

#[test]
fn a_folder_that_is_no_repository_has_no_worktrees() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(worktrees(&LocalProject::open(dir.path()).unwrap()).unwrap(), []);
}

#[test]
fn the_porcelain_list_reads_paths_with_spaces_and_bare_entries() {
    let text = "worktree /r/main\0HEAD 1111111111111111111111111111111111111111\0branch refs/heads/main\0\0\
                worktree /r/with space\0HEAD 2222222222222222222222222222222222222222\0detached\0locked\0\0";
    let listed = parse_list(text);
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[1].path, PathBuf::from("/r/with space"));
    assert_eq!((listed[1].branch.clone(), listed[1].locked.clone()), (None, Some(String::new())));
    assert!(listed[0].main && !listed[1].main);
}
