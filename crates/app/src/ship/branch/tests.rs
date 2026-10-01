use std::{path::Path, process::Command as Git, sync::Arc};

use super::*;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Git::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A clone of a bare remote whose default branch is `main`, as the forge's repositories are.
fn clone() -> (tempfile::TempDir, std::path::PathBuf, Arc<dyn Project>) {
    let top = tempfile::tempdir().unwrap();
    let (bare, work) = (top.path().join("remote.git"), top.path().join("work"));
    git(top.path(), &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]);
    git(top.path(), &["clone", "-q", bare.to_str().unwrap(), work.to_str().unwrap()]);
    for (k, v) in [("user.name", "qa"), ("user.email", "qa@x"), ("commit.gpgsign", "false")] {
        git(&work, &["config", k, v]);
    }
    std::fs::write(work.join("a.txt"), "a\n").unwrap();
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-qm", "start"]);
    git(&work, &["push", "-q", "origin", "main"]);
    git(&work, &["remote", "set-head", "origin", "main"]);
    let project: Arc<dyn Project> = Arc::new(atelier_project::LocalProject::open(&work).unwrap());
    (top, work, project)
}

#[test]
fn the_default_branch_asks_for_a_new_one_and_a_bad_name_is_refused() {
    let (_top, work, project) = clone();
    assert_eq!(current(project.as_ref()).as_deref(), Some("main"));
    assert!(is_default(project.as_ref(), "main"));
    assert!(!is_default(project.as_ref(), "fix/lease"));
    std::fs::write(work.join("a.txt"), "changed\n").unwrap();
    assert!(commit_on_new(project.as_ref(), "bad name..", || Ok(())).is_err());
    commit_on_new(project.as_ref(), "fix/lease", || Ok(())).unwrap();
    assert_eq!(current(project.as_ref()).as_deref(), Some("fix/lease"));
    assert_eq!(std::fs::read_to_string(work.join("a.txt")).unwrap(), "changed\n", "the working tree comes along");
}
/// A commit that fails on the new branch leaves the reader on the branch they were on, and the new
/// branch is gone; the files stay as they are.
#[test]
fn a_failed_commit_on_a_new_branch_changes_nothing() {
    let (_top, work, project) = clone();
    std::fs::write(work.join("a.txt"), "changed\n").unwrap();
    let error = commit_on_new(project.as_ref(), "fix/lease", || Err::<(), _>("the hook said no".to_string())).unwrap_err();
    assert_eq!(error, "the hook said no");
    assert_eq!(current(project.as_ref()).as_deref(), Some("main"));
    assert_eq!(git(&work, &["branch", "--list", "fix/lease"]), "");
    assert_eq!(std::fs::read_to_string(work.join("a.txt")).unwrap(), "changed\n");
    // A name a branch already has is refused before anything moves.
    git(&work, &["branch", "taken"]);
    assert!(commit_on_new(project.as_ref(), "taken", || Ok(())).is_err());
    assert_eq!(current(project.as_ref()).as_deref(), Some("main"));
}
/// A drafted name that a branch already has gets the first free number.
#[test]
fn a_taken_name_gets_a_number() {
    let (_top, work, project) = clone();
    assert_eq!(free(project.as_ref(), "fix/a"), "fix/a");
    git(&work, &["branch", "fix/a"]);
    assert_eq!(free(project.as_ref(), "fix/a"), "fix/a-2");
    git(&work, &["branch", "fix/a-2"]);
    assert_eq!(free(project.as_ref(), "fix/a"), "fix/a-3");
}
/// When HEAD cannot move back after a failed commit, the words say so and where the reader is, and
/// the new branch stays, since HEAD still names it.
#[test]
fn a_failed_move_back_is_named() {
    let (_top, work, project) = clone();
    let error = commit_on_new_with(project.as_ref(), "fix/lease", || Err::<(), _>("the hook said no".to_string()), |_| Err("HEAD is locked".to_string()))
        .unwrap_err();
    assert_eq!(error, "the hook said no, and HEAD could not move back to main: HEAD is locked. You are on fix/lease");
    assert_eq!(current(project.as_ref()).as_deref(), Some("fix/lease"));
    assert_eq!(git(&work, &["branch", "--list", "fix/lease"]).trim(), "* fix/lease", "the branch HEAD names stays");
}
