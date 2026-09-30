//! Runs the Forge write calls on GitHub once, on one scratch repository, `flazouh/lathe-qa-scratch`, through the
//! real `gh`. Ignored by default, and it does nothing unless `LATHE_REQUIRE_FORGE=1`:
//!   LATHE_REQUIRE_FORGE=1 cargo test -p lathe-forge --test live_writes -- --ignored --nocapture
//! It writes to no other repository. Each step is reported on stderr as `ok`, `refused as expected` or `not
//! tested: why`. It makes a branch and a pull request, marks it ready, brings a newer base into it, turns merge when ready
//! on and off where the repository allows it, leaves the queue when it has one, refuses to delete the branch of an open
//! pull request, merges it, deletes its branch, reverts it, closes the revert, and then cleans up: the pull requests are closed, the branches
//! deleted and the files it put on `main` removed. A drop guard does the cleanup even when a step fails.
use std::{path::Path, process::Command, sync::Arc, time::Duration};

use lathe_forge::{
    Forge, ForgeError, MergeMethod, MergeRequest, NewPull, PullRef, PullState, PullUpdate, RepoRef, UpdateMethod,
    github::GitHub,
};
use lathe_project::LocalProject;

const REPO: (&str, &str) = ("flazouh", "lathe-qa-scratch");

fn sh(dir: &Path, program: &str, args: &[&str]) -> String {
    let out = Command::new(program).args(args).current_dir(dir).output().unwrap_or_else(|e| panic!("{program}: {e}"));
    assert!(out.status.success(), "{program} {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn git(dir: &Path, args: &[&str]) -> String {
    sh(dir, "git", args)
}

/// Runs `f` and says how it went, without stopping the run: the point is to see every call once.
fn step<T>(name: &str, f: impl FnOnce() -> Result<T, ForgeError>) -> Option<T> {
    match f() {
        Ok(value) => {
            eprintln!("ok                    {name}");
            Some(value)
        }
        Err(error) => {
            eprintln!("refused / failed      {name}: {error}");
            None
        }
    }
}

/// Everything the run made, so that it is undone even when a step panics.
struct Cleanup<'a> {
    forge: &'a GitHub,
    dir: &'a Path,
    id: String,
    pulls: Vec<PullRef>,
    branches: Vec<String>,
}

impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        for pull in &self.pulls {
            if self.forge.pull(pull).is_ok_and(|p| matches!(p.state, PullState::Open | PullState::Draft)) {
                let _ = self.forge.update_pull(pull, &PullUpdate { closed: Some(true), ..Default::default() });
            }
        }
        for branch in &self.branches {
            let _ = Command::new("git").args(["push", "origin", "--delete", branch]).current_dir(self.dir).output();
        }
        // What the run put on main goes: the files with this run's id.
        let _ = Command::new("git").args(["checkout", "-q", "main"]).current_dir(self.dir).output();
        let _ = Command::new("git").args(["pull", "-q", "--ff-only", "origin", "main"]).current_dir(self.dir).output();
        let files = format!("qa/{}-*", self.id);
        if Command::new("git").args(["rm", "-q", "-r", "--ignore-unmatch", &files]).current_dir(self.dir).output().is_ok() {
            let changed = Command::new("git").args(["diff", "--cached", "--quiet"]).current_dir(self.dir).status().map(|s| !s.success()).unwrap_or(false);
            if changed {
                let _ = Command::new("git").args(["commit", "-q", "-m", &format!("Clean up after the QA run {}", self.id)]).current_dir(self.dir).output();
                let _ = Command::new("git").args(["push", "-q", "origin", "main"]).current_dir(self.dir).output();
            }
        }
        eprintln!("cleaned up            closed {} pull request(s), deleted {} branch(es), removed the run's files from main", self.pulls.len(), self.branches.len());
    }
}

#[test]
#[ignore = "writes to the scratch repository flazouh/lathe-qa-scratch through the real gh"]
fn the_write_calls_run_once_on_the_scratch_repository() {
    if std::env::var("LATHE_REQUIRE_FORGE").as_deref() != Ok("1") {
        eprintln!("skipped: set LATHE_REQUIRE_FORGE=1 to write to the scratch repository");
        return;
    }
    let work = tempfile::tempdir().unwrap();
    let dir = work.path().join("scratch");
    sh(work.path(), "gh", &["repo", "clone", &format!("{}/{}", REPO.0, REPO.1), dir.to_str().unwrap()]);
    // Pushes use gh's credentials, for this clone only: no global git setting is changed.
    git(&dir, &["config", "credential.helper", "!gh auth git-credential"]);
    git(&dir, &["config", "user.name", "lathe qa"]);
    git(&dir, &["config", "user.email", "qa@example.invalid"]);
    // An empty repository has no main yet: make one, with a file that stays.
    if git(&dir, &["branch", "--show-current"]).is_empty() || Command::new("git").args(["rev-parse", "--verify", "-q", "origin/main"]).current_dir(&dir).status().map(|s| !s.success()).unwrap_or(true) {
        git(&dir, &["checkout", "-q", "-B", "main"]);
        std::fs::write(dir.join("README.md"), "A scratch repository for lathe's QA.\n").unwrap();
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", "Start"]);
        git(&dir, &["push", "-q", "-u", "origin", "main"]);
    }
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["pull", "-q", "--ff-only", "origin", "main"]);

    let project = Arc::new(LocalProject::open(&dir).unwrap());
    let forge = GitHub::new(project);
    let repo = RepoRef::new("github.com", REPO.0, REPO.1);
    let id = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
    let mut cleanup = Cleanup { forge: &forge, dir: &dir, id: id.clone(), pulls: Vec::new(), branches: Vec::new() };

    let repository = forge.repository(&format!("https://github.com/{}/{}.git", REPO.0, REPO.1)).unwrap();
    eprintln!("repository            {} default {:?} methods {:?} queue {} auto-merge {}", repository.reference.slug(), repository.default_branch, repository.merge.methods, repository.merge.has_queue, repository.merge.auto_merge_allowed);

    // A branch with one file, and a draft pull request from it.
    let branch = format!("qa-{id}-a");
    git(&dir, &["checkout", "-q", "-b", &branch]);
    std::fs::create_dir_all(dir.join("qa")).unwrap();
    std::fs::write(dir.join(format!("qa/{id}-a.txt")), "a\n").unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "QA: a file"]);
    git(&dir, &["push", "-q", "-u", "origin", &branch]);
    cleanup.branches.push(branch.clone());
    git(&dir, &["checkout", "-q", "main"]);
    let new = NewPull { title: format!("QA {id}: write calls"), body: "Made by lathe's live QA. It closes itself.".into(), base: "main".into(), head: branch.clone(), draft: true };
    let pull = forge.create_pull(&repo, &new).unwrap();
    cleanup.pulls.push(pull.clone());
    eprintln!("ok                    create_pull #{}", pull.number);
    assert_eq!(forge.pull(&pull).unwrap().state, PullState::Draft);

    // Ready for review.
    step("update_pull ready", || forge.update_pull(&pull, &PullUpdate { ready: Some(true), ..Default::default() }));
    assert_eq!(forge.pull(&pull).unwrap().state, PullState::Open, "ready for review: no longer a draft");

    // The branch of an open pull request is not deleted: that would close it.
    let refused = forge.delete_branch(&pull);
    assert!(matches!(&refused, Err(ForgeError::Rejected(why)) if why.contains("still open")), "{refused:?}");
    eprintln!("refused as expected   delete_branch on an open pull request: {}", refused.err().unwrap());

    std::fs::create_dir_all(dir.join("qa")).unwrap();
    // Update branch: main moves on, and the branch takes it in.
    std::fs::write(dir.join(format!("qa/{id}-base.txt")), "base\n").unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "QA: main moves on"]);
    git(&dir, &["push", "-q", "origin", "main"]);
    let before = forge.pull(&pull).unwrap().head_sha;
    let stale = forge.update_branch(&pull, UpdateMethod::Merge, "0000000000000000000000000000000000000000");
    eprintln!("{}   update_branch with a head that moved: {}", if stale.is_err() { "refused as expected" } else { "NOT REFUSED       " }, stale.err().map_or("sent".into(), |e| e.to_string()));
    step("update_branch (merge)", || forge.update_branch(&pull, UpdateMethod::Merge, &before));
    let mut after = before.clone();
    for _ in 0..30 {
        after = forge.pull(&pull).unwrap().head_sha;
        if after != before {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    assert_ne!(after, before, "the branch took main in");
    eprintln!("ok                    the head moved {} -> {}", &before[..7], &after[..7]);

    // Merge when ready, and cancelling it, where the repository allows it (the reader allowed a merge in this repository).
    if repository.merge.auto_merge_allowed {
        let request = MergeRequest { method: MergeMethod::Merge, title: None, message: None, expected_head: Some(after.clone()), when_ready: true, delete_branch: false };
        if let Some(outcome) = step("merge when ready", || forge.merge(&pull, &request)) {
            eprintln!("                      -> {outcome:?}");
            if forge.pull(&pull).unwrap().state == PullState::Open {
                step("cancel_auto_merge", || forge.cancel_auto_merge(&pull));
                assert!(!forge.pull(&pull).unwrap().auto_merge, "merge when ready is off");
            }
        }
    } else {
        eprintln!("not tested            merge when ready, because the repository does not allow auto-merge");
        let alone = forge.cancel_auto_merge(&pull);
        eprintln!("recorded              cancel_auto_merge with none to cancel: {}", alone.err().map_or("ok".to_string(), |e| e.to_string()));
    }

    // The queue.
    if repository.merge.has_queue {
        step("dequeue", || forge.dequeue(&pull));
    } else {
        eprintln!("not tested            dequeue, because the repository has no merge queue");
    }

    // Merge (the reader allowed it in this repository only), then delete its branch and revert it.
    if forge.pull(&pull).unwrap().state != PullState::Merged {
        let request = MergeRequest { method: MergeMethod::Merge, title: None, message: None, expected_head: Some(after.clone()), when_ready: false, delete_branch: false };
        assert!(step("merge", || forge.merge(&pull, &request)).is_some(), "the pull request merges");
    }
    assert_eq!(forge.pull(&pull).unwrap().state, PullState::Merged);
    step("delete_branch after the merge", || forge.delete_branch(&pull));
    let gone = Command::new("gh").args(["api", &format!("repos/{}/{}/git/ref/heads/{branch}", REPO.0, REPO.1)]).output().unwrap();
    assert!(!gone.status.success(), "the branch is gone from GitHub");
    cleanup.branches.retain(|b| b != &branch);
    if let Some(revert) = step("revert", || forge.revert(&pull)) {
        cleanup.pulls.push(revert.clone());
        let made = forge.pull(&revert).unwrap();
        eprintln!("                      the revert is #{} {:?} \"{}\" from {}", revert.number, made.state, made.title, made.head);
        cleanup.branches.push(made.head.clone());
        assert!(made.title.to_lowercase().contains("revert"));
        step("update_pull closed (the revert)", || forge.update_pull(&revert, &PullUpdate { closed: Some(true), ..Default::default() }));
    }

    // A pull request closed without a merge may lose its branch too.
    let other = format!("qa-{id}-b");
    git(&dir, &["checkout", "-q", "-b", &other]);
    std::fs::write(dir.join(format!("qa/{id}-b.txt")), "b\n").unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "QA: another file"]);
    git(&dir, &["push", "-q", "-u", "origin", &other]);
    cleanup.branches.push(other.clone());
    git(&dir, &["checkout", "-q", "main"]);
    let second = forge.create_pull(&repo, &NewPull { title: format!("QA {id}: closed"), body: "Made by lathe's live QA.".into(), base: "main".into(), head: other.clone(), draft: false }).unwrap();
    cleanup.pulls.push(second.clone());
    step("update_pull closed", || forge.update_pull(&second, &PullUpdate { closed: Some(true), ..Default::default() }));
    assert_eq!(forge.pull(&second).unwrap().state, PullState::Closed);
    step("delete_branch after the pull request closed", || forge.delete_branch(&second));
    cleanup.branches.retain(|b| b != &other);
    drop(cleanup);
}
