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
    let mine = git(&work, &["rev-parse", "HEAD"]).trim().to_string();
    let rebased = pull_rebase(project.as_ref(), "main").unwrap();
    let now = git(&work, &["rev-parse", "HEAD"]).trim().to_string();
    assert_eq!(rebased.moved, [(mine, now)], "the commit's old id maps to its new one");
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
/// The reader's own ssh command stays, with no prompt added; plain ssh only when they set none. The
/// environment wins over the config, as in git.
#[test]
fn the_readers_ssh_command_keeps_with_no_prompt() {
    assert_eq!(batch_ssh(None, None), "ssh -o BatchMode=yes");
    assert_eq!(batch_ssh(None, Some("ssh -i ~/.ssh/work")), "ssh -i ~/.ssh/work -o BatchMode=yes");
    assert_eq!(batch_ssh(Some("ssh -J jump"), Some("ssh -i ~/.ssh/work")), "ssh -J jump -o BatchMode=yes");
    assert_eq!(batch_ssh(Some(""), Some("  ")), "ssh -o BatchMode=yes", "empty ones count as none");
}
/// A push runs the ssh command the repository sets, told not to prompt.
#[test]
fn a_push_runs_the_repositorys_ssh_command() {
    let (top, work, _other, project) = remote();
    let (script, called) = (top.path().join("myssh"), top.path().join("called"));
    std::fs::write(&script, format!("#!/bin/sh\necho \"$@\" > {}\nexit 1\n", called.display())).unwrap();
    Git::new("chmod").args(["+x", script.to_str().unwrap()]).status().unwrap();
    git(&work, &["config", "core.sshCommand", &format!("{} -i key", script.display())]);
    git(&work, &["remote", "set-url", "origin", "ssh://git@example.invalid/a/b.git"]);
    assert!(push(project.as_ref(), "main").is_err());
    let args = std::fs::read_to_string(&called).expect("the repository's ssh command ran");
    assert!(args.starts_with("-i key -o BatchMode=yes"), "{args}");
}
/// Set aside on the reader's word: the edits go to a named stash entry, the rebase runs, and the edits
/// come back, leaving no entry.
#[test]
fn edits_set_aside_come_back_after_the_rebase() {
    let (_top, work, other, project) = remote();
    commit(&other, "b.txt", "theirs\n", "Theirs");
    git(&other, &["push", "-q", "origin", "main"]);
    commit(&work, "a.txt", "1\nTWO\n3\n", "Mine");
    std::fs::write(work.join("a.txt"), "1\nTWO\n3\nmore\n").unwrap();
    let rebased = pull_rebase_setting_aside(project.as_ref(), "main").unwrap();
    assert_eq!(rebased.edits, Some(PutBack::Back));
    assert_eq!(std::fs::read_to_string(work.join("a.txt")).unwrap(), "1\nTWO\n3\nmore\n");
    assert_eq!(git(&work, &["stash", "list"]), "", "the entry went back");
    assert_eq!(git(&work, &["log", "--format=%s", "-2"]).lines().collect::<Vec<_>>(), ["Mine", "Theirs"]);
}
/// When the edits clash with the new commits, the rebase stands, the entry stays under its name, and
/// the words name it and the files.
#[test]
fn edits_that_clash_stay_in_their_named_entry() {
    let (_top, work, other, project) = remote();
    commit(&other, "a.txt", "1\n2\nTHREE\n", "Theirs");
    git(&other, &["push", "-q", "origin", "main"]);
    commit(&work, "b.txt", "mine\n", "Mine");
    std::fs::write(work.join("a.txt"), "1\n2\nthree, mine\n").unwrap();
    let rebased = pull_rebase_setting_aside(project.as_ref(), "main").unwrap();
    let Some(PutBack::Kept { entry, files }) = rebased.edits.clone() else { panic!("{rebased:?}") };
    assert_eq!(files, ["a.txt"]);
    assert!(git(&work, &["stash", "list"]).contains(ENTRY_NAME), "the entry stays");
    assert!(entry.starts_with("stash@{"), "{entry}");
    let words = PutBack::Kept { entry, files }.to_string();
    assert!(words.contains("a.txt") && words.contains(ENTRY_NAME), "{words}");
    assert_eq!(git(&work, &["log", "--format=%s", "-2"]).lines().collect::<Vec<_>>(), ["Mine", "Theirs"]);
}
/// Another entry pushed during the rebase is left alone: the pop takes the entry lathe made.
#[test]
fn the_pop_takes_lathes_own_entry() {
    let (_top, work, other, project) = remote();
    commit(&other, "b.txt", "theirs\n", "Theirs");
    git(&other, &["push", "-q", "origin", "main"]);
    commit(&work, "a.txt", "1\nTWO\n3\n", "Mine");
    std::fs::write(work.join("a.txt"), "1\nTWO\n3\nmore\n").unwrap();
    let w = work.clone();
    let during = move || {
        std::fs::write(w.join("c.txt"), "someone else\n").unwrap();
        git(&w, &["add", "c.txt"]);
        git(&w, &["stash", "push", "-q", "-m", "someone else's"]);
    };
    let rebased = pull_rebase_setting_aside_with(project.as_ref(), "main", &during).unwrap();
    assert_eq!(rebased.edits, Some(PutBack::Back));
    assert_eq!(std::fs::read_to_string(work.join("a.txt")).unwrap(), "1\nTWO\n3\nmore\n", "lathe's entry came back");
    let list = git(&work, &["stash", "list"]);
    assert!(list.contains("someone else's") && !list.contains(ENTRY_NAME), "{list}");
}
/// When the rebase stops and the edits cannot go back, the words keep the rebase's reason and name
/// the entry that holds the edits.
#[test]
fn a_stopped_rebase_keeps_its_reason_and_names_the_entry() {
    let (_top, work, other, project) = remote();
    commit(&other, "b.txt", "theirs\n", "Theirs");
    git(&other, &["push", "-q", "origin", "main"]);
    commit(&work, "a.txt", "1\nTWO\n3\n", "Mine");
    std::fs::write(work.join("a.txt"), "1\nTWO\n3\nmore\n").unwrap();
    let w = work.clone();
    // Something writes the same file while the edits are aside: the rebase refuses, and so would a pop.
    let during = move || std::fs::write(w.join("a.txt"), "1\nTWO\n3\nsomething else\n").unwrap();
    let error = pull_rebase_setting_aside_with(project.as_ref(), "main", &during).unwrap_err();
    let RebaseError::EditsKept { why, entry } = &error else { panic!("{error:?}") };
    assert!(!why.is_empty() && entry.starts_with("stash@{"), "{error:?}");
    let words = error.to_string();
    assert!(words.contains(why.as_str()) && words.contains(ENTRY_NAME), "{words}");
    assert!(git(&work, &["stash", "list"]).contains(ENTRY_NAME), "the edits stay in the entry");
}
/// In tests, a call that reaches a remote goes only to a folder on this machine or to a host that can
/// never resolve: a test that points at GitHub stops before git runs.
#[test]
#[should_panic(expected = "a test reached for a network remote")]
fn a_test_never_reaches_a_real_remote() {
    let (_top, work, _other, project) = remote();
    git(&work, &["remote", "set-url", "origin", "https://github.com/flazouh/lathe-qa-scratch.git"]);
    let _ = push(project.as_ref(), "main");
}
