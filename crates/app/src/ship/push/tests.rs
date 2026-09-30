use std::{path::Path, process::Command as Git, sync::Arc};
use lathe_project::{LocalProject, Project};
use super::*;
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Git::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}
fn configure(dir: &Path) {
    for (k, v) in [("user.name", "qa"), ("user.email", "qa@x"), ("commit.gpgsign", "false")] {
        git(dir, &["config", k, v]);
    }
}
/// A bare remote with one commit on `main`, and two clones of it: the reader's and another's.
fn remote() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf, Arc<dyn Project>) {
    let top = tempfile::tempdir().unwrap();
    let (bare, work, other) = (top.path().join("remote.git"), top.path().join("work"), top.path().join("other"));
    git(top.path(), &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]);
    git(top.path(), &["clone", "-q", bare.to_str().unwrap(), work.to_str().unwrap()]);
    configure(&work);
    std::fs::write(work.join("a.txt"), "1\n2\n3\n").unwrap();
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-qm", "start"]);
    git(&work, &["push", "-q", "origin", "main"]);
    git(top.path(), &["clone", "-q", bare.to_str().unwrap(), other.to_str().unwrap()]);
    configure(&other);
    let project: Arc<dyn Project> = Arc::new(LocalProject::open(&work).unwrap());
    (top, work, other, project)
}
fn commit(dir: &Path, file: &str, text: &str, message: &str) {
    std::fs::write(dir.join(file), text).unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", message]);
}
/// A new branch goes to origin and tracks it.
#[test]
fn a_new_branch_is_pushed_and_tracks_origin() {
    let (_top, work, _other, project) = remote();
    git(&work, &["switch", "-qc", "fix/two"]);
    commit(&work, "a.txt", "1\nTWO\n3\n", "Two");
    push(project.as_ref(), "fix/two").unwrap();
    assert_eq!(git(&work, &["rev-parse", "--abbrev-ref", "fix/two@{u}"]).trim(), "origin/fix/two");
    assert_eq!(git(&work, &["rev-parse", "origin/fix/two"]), git(&work, &["rev-parse", "HEAD"]));
}
/// A branch the remote moved on is rejected, in plain words and never forced; pull and rebase puts
/// the reader's commit on top, and the push then goes through with both commits on the remote.
#[test]
fn a_rejected_push_pulls_and_rebases_then_pushes() {
    let (_top, work, other, project) = remote();
    commit(&other, "b.txt", "theirs\n", "Theirs");
    git(&other, &["push", "-q", "origin", "main"]);
    commit(&work, "a.txt", "1\nTWO\n3\n", "Mine");
    let error = push(project.as_ref(), "main").unwrap_err();
    assert_eq!(error, PushError::Rejected);
    assert!(error.to_string().contains("Pull and rebase"), "{error}");
    pull_rebase(project.as_ref(), "main").unwrap();
    push(project.as_ref(), "main").unwrap();
    let log = git(&work, &["log", "--format=%s", "origin/main"]);
    assert_eq!(log.lines().collect::<Vec<_>>(), ["Mine", "Theirs", "start"]);
}
/// A rebase that meets a conflict is undone: the branch and the files are as they were, and the words
/// name the file.
#[test]
fn a_rebase_conflict_is_undone_and_named() {
    let (_top, work, other, project) = remote();
    commit(&other, "a.txt", "1\nTHEIRS\n3\n", "Theirs");
    git(&other, &["push", "-q", "origin", "main"]);
    commit(&work, "a.txt", "1\nMINE\n3\n", "Mine");
    let before = git(&work, &["rev-parse", "HEAD"]);
    let error = pull_rebase(project.as_ref(), "main").unwrap_err();
    assert_eq!(error, RebaseError::Conflict(vec!["a.txt".into()]));
    assert!(error.to_string().contains("a.txt"), "{error}");
    assert_eq!(git(&work, &["rev-parse", "HEAD"]), before, "the branch is as it was");
    assert!(!work.join(".git/rebase-merge").exists() && !work.join(".git/rebase-apply").exists(), "no rebase is left open");
    assert_eq!(std::fs::read_to_string(work.join("a.txt")).unwrap(), "1\nMINE\n3\n");
}
/// A repository with no remote says so before it asks git to push.
#[test]
fn no_remote_says_so() {
    let (_top, work, _other, project) = remote();
    git(&work, &["remote", "remove", "origin"]);
    assert_eq!(push(project.as_ref(), "main").unwrap_err(), PushError::NoRemote);
}
/// An unreachable remote reads as offline, and git never waits on a login prompt.
#[test]
fn an_unreachable_remote_reads_as_offline() {
    let (_top, work, _other, project) = remote();
    git(&work, &["remote", "set-url", "origin", "https://lathe-qa.invalid/nobody/nothing.git"]);
    assert!(matches!(push(project.as_ref(), "main").unwrap_err(), PushError::Offline(_)));
}
/// What git says, sorted into what the reader does next.
#[test]
fn git_words_sort_into_their_kinds() {
    let rejected = " ! [rejected]        main -> main (fetch first)\nerror: failed to push some refs\nhint: Updates were rejected because the remote contains work";
    assert_eq!(classify(rejected), PushError::Rejected);
    assert_eq!(classify(" ! [rejected]        main -> main (non-fast-forward)"), PushError::Rejected);
    assert!(matches!(classify("fatal: unable to access 'https://github.com/a/b.git/': Could not resolve host: github.com"), PushError::Offline(_)));
    assert!(matches!(classify("ssh: connect to host github.com port 22: Network is unreachable"), PushError::Offline(_)));
    assert!(matches!(classify("remote: Permission to a/b.git denied to someone.\nfatal: unable to access 'https://github.com/a/b.git/': The requested URL returned error: 403"), PushError::Login(_)));
    assert!(matches!(classify("fatal: could not read Username for 'https://github.com': terminal prompts disabled"), PushError::Login(_)));
    assert!(matches!(classify("remote: error: GH013: Repository rule violations found"), PushError::Git(_)));
}
/// Pull and rebase with other edits in the working tree leaves them alone and says why it stopped.
#[test]
fn a_rebase_with_other_edits_stops_and_leaves_them() {
    let (_top, work, other, project) = remote();
    commit(&other, "b.txt", "theirs\n", "Theirs");
    git(&other, &["push", "-q", "origin", "main"]);
    commit(&work, "a.txt", "1\nTWO\n3\n", "Mine");
    std::fs::write(work.join("a.txt"), "1\nTWO\n3\nmore\n").unwrap();
    assert_eq!(pull_rebase(project.as_ref(), "main").unwrap_err(), RebaseError::OtherEdits);
    assert_eq!(std::fs::read_to_string(work.join("a.txt")).unwrap(), "1\nTWO\n3\nmore\n");
    assert_eq!(git(&work, &["stash", "list"]), "", "nothing was put aside");
}
