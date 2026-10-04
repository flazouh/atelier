use std::sync::Mutex;

use atelier_forge::*;

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
    fn briefs(&self, _: &RepoRef, numbers: &[u64]) -> ForgeResult<Vec<Option<PullSummary>>> {
        self.briefs_asked.lock().unwrap().push(numbers.to_vec());
        if let Some(error) = self.reads_fail.lock().unwrap().clone() {
            return Err(error);
        }
        let known = self.known.lock().unwrap();
        let summary = |brief: &PullBrief| PullSummary {
            brief: brief.clone(),
            author: "a".into(),
            created_at: 0,
            updated_at: 0,
            additions: 0,
            deletions: 0,
            comments: 0,
            review: atelier_forge::ReviewDecision::NotRequired,
            checks: None,
        };
        Ok(numbers.iter().map(|n| known.iter().find(|b| b.reference.number == *n).map(summary)).collect())
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
