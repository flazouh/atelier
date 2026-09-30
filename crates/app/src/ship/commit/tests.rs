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

/// What a commit of 200 changed files costs, on a local project and, with LATHE_TEST_SSH_HOST (and
/// LATHE_REMOTE_DIR), over ssh to that host. docs/performance.md, "Commit".
///     cargo test --release -p lathe-app -- --ignored --nocapture commit_of_200_files
#[test]
#[ignore]
fn commit_of_200_files() {
    let files: Vec<(String, String)> = (0..200).map(|i| (format!("f{i:03}.txt"), format!("line {i}\n"))).collect();
    let refs: Vec<(&str, &str)> = files.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
    let kept: Vec<Kept> = files.iter().map(|(p, _)| write(p, "changed\n")).collect();
    let time = |project: &dyn Project, runs: usize| {
        let mut took: Vec<f64> = (0..runs)
            .map(|run| {
                let kept: Vec<Kept> = kept.iter().map(|k| write(&k.path, &format!("changed {run}\n"))).collect();
                let at = std::time::Instant::now();
                commit(project, &kept, &format!("run {run}")).unwrap_or_else(|e| panic!("commit: {e}"));
                at.elapsed().as_secs_f64() * 1000.
            })
            .collect();
        took.sort_by(f64::total_cmp);
        (took[took.len() / 2], took[took.len() - 1])
    };
    let (_dir, project) = repo(&refs);
    let (median, worst) = time(project.as_ref(), 5);
    println!("local, 200 files: median {median:.0} ms, worst {worst:.0} ms (5 runs)");
    if let Ok(host) = std::env::var("LATHE_TEST_SSH_HOST") {
        let (dir, _) = repo(&refs);
        let remote = lathe_remote::ssh::connect(&host, &dir.path().display().to_string(), &|_| {}).unwrap_or_else(|e| panic!("no connection: {e}"));
        let (median, worst) = time(&remote, 3);
        println!("over ssh to {host}, 200 files: median {median:.0} ms, worst {worst:.0} ms (3 runs)");
    }
}

/// The branch moving between the index read from HEAD and the commit (the reader committed in a
/// terminal meanwhile) must not undo that commit: the commit is refused, and nothing changes.
#[test]
fn a_branch_that_moves_while_committing_refuses_the_commit() {
    let (dir, project) = repo(&[("a.txt", "a\n"), ("b.txt", "b\n")]);
    let moved = || {
        std::fs::write(dir.path().join("b.txt"), "b from the terminal\n").unwrap();
        git(dir.path(), &["commit", "-qam", "from the terminal"]);
    };
    match commit_with(project.as_ref(), &[write("a.txt", "A\n")], "Keep A", &moved) {
        Err(CommitError::Moved) => {}
        other => panic!("wrong answer: {other:?}"),
    }
    assert_eq!(git(dir.path(), &["log", "-1", "--format=%s"]).trim(), "from the terminal", "the terminal's commit stands");
    assert_eq!(git(dir.path(), &["show", "HEAD:b.txt"]), "b from the terminal\n");
    assert!(git(dir.path(), &["diff", "--cached", "--name-only"]).trim().is_empty());
}

/// A hook that says a lot on stderr (a linter over the whole tree) does not stall the commit.
#[test]
fn a_hook_that_writes_a_megabyte_to_stderr_does_not_stall() {
    let (dir, project) = repo(&[("a.txt", "a\n")]);
    let hook = dir.path().join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\nhead -c 1048576 /dev/zero | tr '\\0' x >&2\nexit 0\n").unwrap();
    Git::new("chmod").args(["+x", hook.to_str().unwrap()]).status().unwrap();
    commit(project.as_ref(), &[write("a.txt", "A\n")], "Keep A").unwrap();
    assert_eq!(git(dir.path(), &["show", "HEAD:a.txt"]), "A\n");
}
