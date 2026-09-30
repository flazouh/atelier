//! A forge for tests: it keeps what it was asked to write and answers from a script, and a repository
//! on disk with a pushed branch to open a pull request from. Nothing here reaches a network.
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};
use lathe_forge::*;

#[derive(Default)]
pub struct FakeForge {
    /// Each pull request asked for, with its repository.
    pub created: Mutex<Vec<(RepoRef, NewPull)>>,
    /// The error the next writes answer with, if any.
    pub fails: Mutex<Option<ForgeError>>,
    /// The open pull request a branch already has, as `open_pull_for` answers.
    pub open: Mutex<Option<PullBrief>>,
    /// The error `open_pull_for` and `briefs` answer with, if any.
    pub reads_fail: Mutex<Option<ForgeError>>,
    /// The pull requests `briefs` knows, by number, and each batch of numbers it was asked for.
    pub known: Mutex<Vec<PullBrief>>,
    pub briefs_asked: Mutex<Vec<Vec<u64>>>,
}

impl FakeForge {
    fn answer<T>(&self, ok: T) -> ForgeResult<T> {
        match self.fails.lock().unwrap().clone() {
            Some(error) => Err(error),
            None => Ok(ok),
        }
    }
}

impl Forge for FakeForge {
    fn create_pull(&self, repo: &RepoRef, new: &NewPull) -> ForgeResult<PullRef> {
        let reference = self.answer(PullRef { repo: repo.clone(), number: 7 })?;
        self.created.lock().unwrap().push((repo.clone(), new.clone()));
        Ok(reference)
    }
    fn open_pull_for(&self, _: &RepoRef, _: &str) -> ForgeResult<Option<PullBrief>> {
        if let Some(error) = self.reads_fail.lock().unwrap().clone() {
            return Err(error);
        }
        Ok(self.open.lock().unwrap().clone())
    }
    fn repository(&self, _: &str) -> ForgeResult<Repository> { unimplemented!() }
    fn pull(&self, _: &PullRef) -> ForgeResult<Pull> { unimplemented!() }
    fn files(&self, _: &PullRef) -> ForgeResult<Vec<ChangedFile>> { unimplemented!() }
    fn threads(&self, _: &PullRef) -> ForgeResult<Vec<Thread>> { unimplemented!() }
    fn remarks(&self, _: &PullRef) -> ForgeResult<Vec<Comment>> { unimplemented!() }
    fn checks(&self, _: &PullRef) -> ForgeResult<Vec<Check>> { unimplemented!() }
    fn job(&self, _: &JobRef) -> ForgeResult<Job> { unimplemented!() }
    fn job_log(&self, _: &JobRef) -> ForgeResult<String> { unimplemented!() }
    fn last_review_point(&self, _: &PullRef) -> ForgeResult<Option<String>> { unimplemented!() }
    fn involved(&self) -> ForgeResult<Vec<Involved>> { unimplemented!() }
    fn briefs(&self, _: &RepoRef, numbers: &[u64]) -> ForgeResult<Vec<Option<PullBrief>>> {
        self.briefs_asked.lock().unwrap().push(numbers.to_vec());
        if let Some(error) = self.reads_fail.lock().unwrap().clone() {
            return Err(error);
        }
        let known = self.known.lock().unwrap();
        Ok(numbers.iter().map(|n| known.iter().find(|b| b.reference.number == *n).cloned()).collect())
    }
    fn update_pull(&self, _: &PullRef, _: &PullUpdate) -> ForgeResult<()> { unimplemented!() }
    fn merge(&self, _: &PullRef, _: &MergeRequest) -> ForgeResult<MergeOutcome> { unimplemented!() }
    fn request_review(&self, _: &PullRef, _: &[Reviewer]) -> ForgeResult<()> { unimplemented!() }
    fn comment(&self, _: &PullRef, _: &str) -> ForgeResult<Comment> { unimplemented!() }
    fn hold_comment(&self, _: &PullRef, _: &NewLine) -> ForgeResult<HeldComment> { unimplemented!() }
    fn held_comments(&self, _: &PullRef) -> ForgeResult<Vec<HeldComment>> { unimplemented!() }
    fn submit_review(&self, _: &PullRef, _: Verdict, _: &str) -> ForgeResult<()> { unimplemented!() }
    fn reply(&self, _: &ThreadId, _: &str) -> ForgeResult<Comment> { unimplemented!() }
    fn resolve(&self, _: &ThreadId, _: bool) -> ForgeResult<()> { unimplemented!() }
}

pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(["-c", "user.name=q", "-c", "user.email=q@q", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A clone of a bare remote with `main` and `release`, on `fix/two` with one commit, pushed. Then
/// origin's address is made a GitHub one, which names the repository, and an `insteadOf` rule sends
/// every fetch and push back to the bare remote, so GitHub is never reached.
pub fn pushed_branch() -> PathBuf {
    let top = crate::test_dirs::path();
    let (bare, work) = (top.join("remote.git"), top.join("work"));
    git(&top, &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]);
    git(&top, &["clone", "-q", bare.to_str().unwrap(), work.to_str().unwrap()]);
    std::fs::write(work.join("a.txt"), "a\n").unwrap();
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-qm", "start"]);
    git(&work, &["push", "-q", "origin", "main", "main:release"]);
    git(&work, &["remote", "set-head", "origin", "main"]);
    git(&work, &["switch", "-qc", "fix/two"]);
    std::fs::write(work.join("a.txt"), "two\n").unwrap();
    git(&work, &["commit", "-qam", "Make a two"]);
    git(&work, &["push", "-q", "-u", "origin", "fix/two"]);
    git(&work, &["remote", "set-url", "origin", SCRATCH_URL]);
    git(&work, &["config", &format!("url.{}.insteadOf", bare.display()), SCRATCH_URL]);
    work
}

/// The scratch repository's address, as the fixture's origin has it.
pub const SCRATCH_URL: &str = "https://github.com/flazouh/lathe-qa-scratch.git";

/// The bare remote behind `work`'s origin.
pub fn bare_of(work: &Path) -> PathBuf {
    work.parent().unwrap().join("remote.git")
}
