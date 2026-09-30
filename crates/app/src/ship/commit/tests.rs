use std::{path::Path, process::Command as Git, sync::Arc};

use super::*;
use crate::ship::kept::Kept;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Git::new("git").args(["-c", "commit.gpgsign=false"]).args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A repository with `files` committed, and a name to commit as.
fn repo(files: &[(&str, &str)]) -> (tempfile::TempDir, Arc<dyn Project>) {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["config", "user.name", "qa"]);
    git(dir.path(), &["config", "user.email", "qa@x"]);
    git(dir.path(), &["config", "commit.gpgsign", "false"]);
    for (path, text) in files {
        std::fs::write(dir.path().join(path), text).unwrap();
    }
    if !files.is_empty() {
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-qm", "start"]);
    }
    let project: Arc<dyn Project> = Arc::new(lathe_project::LocalProject::open(dir.path()).unwrap());
    (dir, project)
}

fn write(path: &str, text: &str) -> Kept {
    Kept { path: path.into(), text: Some(text.into()) }
}

/// The commit takes what the review kept, not the disk: the reader's other staged work stays staged and
/// out of the commit, and the working tree is left as it is.
#[test]
fn a_commit_takes_what_was_kept_and_leaves_the_rest() {
    let (dir, project) = repo(&[("a.txt", "1\n2\n"), ("b.txt", "b\n")]);
    std::fs::write(dir.path().join("a.txt"), "1\nTWO\nundecided\n").unwrap();
    std::fs::write(dir.path().join("b.txt"), "b staged\n").unwrap();
    git(dir.path(), &["add", "b.txt"]);
    let done = commit(project.as_ref(), &[write("a.txt", "1\nTWO\n"), write("new.txt", "n\n")], "Keep TWO").unwrap();
    assert_eq!(git(dir.path(), &["show", "HEAD:a.txt"]), "1\nTWO\n");
    assert_eq!(git(dir.path(), &["show", "HEAD:new.txt"]), "n\n");
    assert_eq!(git(dir.path(), &["show", "HEAD:b.txt"]), "b\n", "the reader's staged work is not in it");
    assert_eq!(git(dir.path(), &["log", "-1", "--format=%s"]).trim(), "Keep TWO");
    assert_eq!(done.sha, git(dir.path(), &["rev-parse", "HEAD"]).trim());
    assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).unwrap(), "1\nTWO\nundecided\n", "the disk is left as it is");
    let staged = git(dir.path(), &["diff", "--cached", "--name-only"]);
    assert_eq!(staged.trim(), "b.txt", "b.txt stays staged; the committed files are clean in the index");
}

/// A removal the review kept goes, in a repository with no commit yet too.
#[test]
fn a_removal_and_a_first_commit() {
    let (dir, project) = repo(&[("gone.txt", "g\n")]);
    commit(project.as_ref(), &[Kept { path: "gone.txt".into(), text: None }], "Remove it").unwrap();
    assert!(git(dir.path(), &["ls-tree", "--name-only", "HEAD"]).trim().is_empty());
    let (fresh, project) = repo(&[]);
    commit(project.as_ref(), &[write("first.txt", "1\n")], "First").unwrap();
    assert_eq!(git(fresh.path(), &["show", "HEAD:first.txt"]), "1\n");
}

/// The repository's hooks run: a pre-commit hook that refuses stops the commit, says its words, and
/// changes nothing.
#[test]
fn a_refusing_hook_stops_the_commit_and_changes_nothing() {
    let (dir, project) = repo(&[("a.txt", "a\n")]);
    let hook = dir.path().join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\necho 'lint failed: a.txt' >&2\nexit 1\n").unwrap();
    Git::new("chmod").args(["+x", hook.to_str().unwrap()]).status().unwrap();
    let head = git(dir.path(), &["rev-parse", "HEAD"]);
    match commit(project.as_ref(), &[write("a.txt", "A\n")], "Try") {
        Err(CommitError::Refused(words)) => assert!(words.contains("lint failed: a.txt"), "{words}"),
        other => panic!("wrong answer: {other:?}"),
    }
    assert_eq!(git(dir.path(), &["rev-parse", "HEAD"]), head, "no commit");
    assert!(git(dir.path(), &["diff", "--cached", "--name-only"]).trim().is_empty(), "the index is as it was");
}

#[test]
fn nothing_kept_is_no_commit() {
    let (_dir, project) = repo(&[("a.txt", "a\n")]);
    assert!(matches!(commit(project.as_ref(), &[], "x"), Err(CommitError::Nothing)));
}
