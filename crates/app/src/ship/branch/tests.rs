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
    let project: Arc<dyn Project> = Arc::new(lathe_project::LocalProject::open(&work).unwrap());
    (top, work, project)
}

#[test]
fn the_default_branch_asks_for_a_new_one_and_a_bad_name_is_refused() {
    let (_top, work, project) = clone();
    assert_eq!(current(project.as_ref()).as_deref(), Some("main"));
    assert!(is_default(project.as_ref(), "main"));
    assert!(!is_default(project.as_ref(), "fix/lease"));
    std::fs::write(work.join("a.txt"), "changed\n").unwrap();
    assert!(create(project.as_ref(), "bad name..").is_err());
    create(project.as_ref(), "fix/lease").unwrap();
    assert_eq!(current(project.as_ref()).as_deref(), Some("fix/lease"));
    assert_eq!(std::fs::read_to_string(work.join("a.txt")).unwrap(), "changed\n", "the working tree comes along");
}
