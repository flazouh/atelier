//! A forge that lives in memory, for tests, the gallery story and the numbers. It holds pull requests
//! as [`PullData`], answers the reads from them, and applies the writes to them, so a test can send a reply
//! and read the thread back. Every call is logged. It sends nothing anywhere.
use std::{
    collections::{HashMap, VecDeque},
    sync::{Mutex, MutexGuard},
};

use lathe_forge::{
    Author, ChangedFile, Check, Comment, Forge, ForgeError, ForgeResult, HeldComment, Involved, Job, JobRef, MergeOutcome,
    MergeRequest, MergeSettings, NewLine, NewPull, Opinion, Pull, PullBrief, PullRef, PullState, PullUpdate, Remark, RepoRef,
    Repository, ReviewDecision, Reviewer, Thread, ThreadId, Verdict,
};

use crate::data::PullData;

pub mod relay;
pub mod repo;

/// A change the reader made, as the forge got it.
#[derive(Clone, Debug, PartialEq)]
pub enum Write {
    Comment { reference: PullRef, body: String },
    Reply { thread: ThreadId, body: String },
    Resolve { thread: ThreadId, resolved: bool },
    Hold(NewLine),
    Submit { reference: PullRef, verdict: Verdict, body: String },
    Merge { reference: PullRef, request: MergeRequest },
    RequestReview { reference: PullRef, reviewers: Vec<Reviewer> },
    Update { reference: PullRef, update: PullUpdate },
    Create(NewPull),
}

#[derive(Default)]
struct State {
    pulls: HashMap<PullRef, PullData>,
    involved: Vec<Involved>,
    repository: Option<Repository>,
    jobs: HashMap<u64, (Job, String)>,
    failures: HashMap<&'static str, VecDeque<ForgeError>>,
    calls: Vec<&'static str>,
    writes: Vec<Write>,
    next: u64,
}

#[derive(Default)]
pub struct FixtureForge {
    state: Mutex<State>,
}

impl FixtureForge {
    pub fn new() -> Self {
        Self::default()
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn with_pull(self, data: PullData) -> Self {
        self.state().pulls.insert(data.reference.clone(), data);
        self
    }

    pub fn with_involved(self, involved: Vec<Involved>) -> Self {
        self.state().involved = involved;
        self
    }

    pub fn with_repository(self, repository: Repository) -> Self {
        self.state().repository = Some(repository);
        self
    }

    /// A job with its steps and its log, for a check that carries a `JobRef`.
    pub fn with_job(self, job: Job, log: impl Into<String>) -> Self {
        self.state().jobs.insert(job.reference.id, (job, log.into()));
        self
    }

    /// The next call named `call` (`"pull"`, `"threads"`, ...) fails with `error`.
    pub fn fail(&self, call: &'static str, error: ForgeError) {
        self.state().failures.entry(call).or_default().push_back(error);
    }

    /// Changes a pull request behind the reader's back, as a push or another person's comment does.
    pub fn edit(&self, reference: &PullRef, change: impl FnOnce(&mut PullData)) {
        if let Some(data) = self.state().pulls.get_mut(reference) {
            change(data);
        }
    }

    /// The reads and writes so far, in order.
    pub fn calls(&self) -> Vec<&'static str> {
        self.state().calls.clone()
    }

    pub fn count(&self, call: &str) -> usize {
        self.state().calls.iter().filter(|c| **c == call).count()
    }

    pub fn writes(&self) -> Vec<Write> {
        self.state().writes.clone()
    }

    pub fn data(&self, reference: &PullRef) -> Option<PullData> {
        self.state().pulls.get(reference).cloned()
    }

    /// Logs the call and fails it if a failure is queued.
    fn enter(&self, call: &'static str) -> ForgeResult<MutexGuard<'_, State>> {
        let mut state = self.state();
        state.calls.push(call);
        match state.failures.get_mut(call).and_then(VecDeque::pop_front) {
            Some(error) => Err(error),
            None => Ok(state),
        }
    }
}

fn missing(reference: &PullRef) -> ForgeError {
    ForgeError::NotFound(format!("{}#{}", reference.repo.slug(), reference.number))
}

impl Forge for FixtureForge {
    fn repository(&self, remote_url: &str) -> ForgeResult<Repository> {
        let state = self.enter("repository")?;
        state.repository.clone().ok_or_else(|| ForgeError::UnknownRemote(remote_url.to_string()))
    }

    fn pull(&self, reference: &PullRef) -> ForgeResult<Pull> {
        let state = self.enter("pull")?;
        state.pulls.get(reference).and_then(|d| d.pull.clone()).ok_or_else(|| missing(reference))
    }

    fn files(&self, reference: &PullRef) -> ForgeResult<Vec<ChangedFile>> {
        let state = self.enter("files")?;
        state.pulls.get(reference).map(|d| d.files.clone()).ok_or_else(|| missing(reference))
    }

    fn threads(&self, reference: &PullRef) -> ForgeResult<Vec<Thread>> {
        let state = self.enter("threads")?;
        state.pulls.get(reference).map(|d| d.threads.clone()).ok_or_else(|| missing(reference))
    }

    fn remarks(&self, reference: &PullRef) -> ForgeResult<Vec<Remark>> {
        let state = self.enter("remarks")?;
        state.pulls.get(reference).map(|d| d.remarks.clone()).ok_or_else(|| missing(reference))
    }

    fn checks(&self, reference: &PullRef) -> ForgeResult<Vec<Check>> {
        let state = self.enter("checks")?;
        state.pulls.get(reference).map(|d| d.checks.clone()).ok_or_else(|| missing(reference))
    }

    fn job(&self, job: &JobRef) -> ForgeResult<Job> {
        let state = self.enter("job")?;
        state.jobs.get(&job.id).map(|(j, _)| j.clone()).ok_or_else(|| ForgeError::NotFound(format!("job {}", job.id)))
    }

    fn job_log(&self, job: &JobRef) -> ForgeResult<String> {
        let state = self.enter("job_log")?;
        state.jobs.get(&job.id).map(|(_, log)| log.clone()).ok_or_else(|| ForgeError::NotFound(format!("the log of job {}", job.id)))
    }

    fn last_review_point(&self, reference: &PullRef) -> ForgeResult<Option<String>> {
        let state = self.enter("last_review_point")?;
        state.pulls.get(reference).map(|d| d.review_point.clone()).ok_or_else(|| missing(reference))
    }

    fn involved(&self) -> ForgeResult<Vec<Involved>> {
        let state = self.enter("involved")?;
        Ok(state.involved.clone())
    }

    fn briefs(&self, repo: &RepoRef, numbers: &[u64]) -> ForgeResult<Vec<Option<PullBrief>>> {
        let state = self.enter("briefs")?;
        Ok(numbers
            .iter()
            .map(|n| {
                let reference = PullRef { repo: repo.clone(), number: *n };
                let pull = state.pulls.get(&reference)?.pull.as_ref()?;
                Some(PullBrief { reference, title: pull.title.clone(), state: pull.state, url: pull.url.clone() })
            })
            .collect())
    }

    fn create_pull(&self, repo: &RepoRef, new: &NewPull) -> ForgeResult<PullRef> {
        let mut state = self.enter("create_pull")?;
        state.writes.push(Write::Create(new.clone()));
        state.next += 1;
        Ok(PullRef { repo: repo.clone(), number: 9000 + state.next })
    }

    fn update_pull(&self, reference: &PullRef, update: &PullUpdate) -> ForgeResult<()> {
        let mut state = self.enter("update_pull")?;
        state.writes.push(Write::Update { reference: reference.clone(), update: update.clone() });
        let pull = state.pulls.get_mut(reference).and_then(|d| d.pull.as_mut()).ok_or_else(|| missing(reference))?;
        if let Some(title) = &update.title {
            pull.title = title.clone();
        }
        if let Some(body) = &update.body {
            pull.body = body.clone();
        }
        if let Some(ready) = update.ready {
            pull.state = if ready { PullState::Open } else { PullState::Draft };
        }
        if let Some(closed) = update.closed {
            pull.state = if closed { PullState::Closed } else { PullState::Open };
        }
        Ok(())
    }

    fn merge(&self, reference: &PullRef, request: &MergeRequest) -> ForgeResult<MergeOutcome> {
        let mut state = self.enter("merge")?;
        state.writes.push(Write::Merge { reference: reference.clone(), request: request.clone() });
        let pull = state.pulls.get_mut(reference).and_then(|d| d.pull.as_mut()).ok_or_else(|| missing(reference))?;
        if request.expected_head.as_deref().is_some_and(|h| h != pull.head_sha) {
            return Err(ForgeError::Rejected("The branch moved since you looked. Read it again.".into()));
        }
        pull.state = PullState::Merged;
        Ok(MergeOutcome::Merged)
    }

    fn request_review(&self, reference: &PullRef, reviewers: &[Reviewer]) -> ForgeResult<()> {
        let mut state = self.enter("request_review")?;
        state.writes.push(Write::RequestReview { reference: reference.clone(), reviewers: reviewers.to_vec() });
        Ok(())
    }

    fn comment(&self, reference: &PullRef, body: &str) -> ForgeResult<Comment> {
        let mut state = self.enter("comment")?;
        state.writes.push(Write::Comment { reference: reference.clone(), body: body.to_string() });
        state.next += 1;
        let comment = Comment { id: format!("c{}", state.next), author: "you".into(), kind: Author::Person, body: body.to_string(), created_at: 1_790_000_000 + state.next, unsent: false };
        let data = state.pulls.get_mut(reference).ok_or_else(|| missing(reference))?;
        data.remarks.push(comment.clone());
        Ok(comment)
    }

    fn hold_comment(&self, reference: &PullRef, comment: &NewLine) -> ForgeResult<HeldComment> {
        let mut state = self.enter("hold_comment")?;
        state.writes.push(Write::Hold(comment.clone()));
        state.next += 1;
        let held = HeldComment {
            thread: ThreadId(format!("held-{}", state.next)),
            comment: Comment { id: format!("h{}", state.next), author: "you".into(), kind: Author::Person, body: comment.body.clone(), created_at: 1_790_000_000 + state.next, unsent: true },
            path: comment.path.clone(),
            line: Some(comment.line),
        };
        state.pulls.get_mut(reference).ok_or_else(|| missing(reference))?.held.push(held.clone());
        Ok(held)
    }

    fn held_comments(&self, reference: &PullRef) -> ForgeResult<Vec<HeldComment>> {
        let state = self.enter("held_comments")?;
        state.pulls.get(reference).map(|d| d.held.clone()).ok_or_else(|| missing(reference))
    }

    fn submit_review(&self, reference: &PullRef, verdict: Verdict, body: &str) -> ForgeResult<()> {
        let mut state = self.enter("submit_review")?;
        state.writes.push(Write::Submit { reference: reference.clone(), verdict, body: body.to_string() });
        let data = state.pulls.get_mut(reference).ok_or_else(|| missing(reference))?;
        for held in std::mem::take(&mut data.held) {
            let mut comment = held.comment;
            comment.unsent = false;
            data.threads.push(Thread {
                id: held.thread,
                resolved: false,
                outdated: false,
                path: held.path,
                file_level: held.line.is_none(),
                line: held.line,
                start_line: None,
                original_line: held.line,
                side: lathe_forge::Side::Right,
                can_resolve: true,
                can_reply: true,
                comments: vec![comment],
            });
        }
        if let Some(pull) = data.pull.as_mut() {
            pull.opinions.retain(|o| o.reviewer != "you");
            pull.opinions.push(Opinion { reviewer: "you".into(), verdict });
            match verdict {
                Verdict::Approve => pull.review = ReviewDecision::Approved,
                Verdict::RequestChanges => pull.review = ReviewDecision::ChangesRequested,
                Verdict::Comment => {}
            }
        }
        Ok(())
    }

    fn reply(&self, thread: &ThreadId, body: &str) -> ForgeResult<Comment> {
        let mut state = self.enter("reply")?;
        state.writes.push(Write::Reply { thread: thread.clone(), body: body.to_string() });
        state.next += 1;
        let comment = Comment { id: format!("r{}", state.next), author: "you".into(), kind: Author::Person, body: body.to_string(), created_at: 1_790_000_000 + state.next, unsent: false };
        let found = state.pulls.values_mut().flat_map(|d| d.threads.iter_mut()).find(|t| t.id == *thread);
        match found {
            Some(t) => {
                t.comments.push(comment.clone());
                Ok(comment)
            }
            None => Err(ForgeError::NotFound("that thread".into())),
        }
    }

    fn resolve(&self, thread: &ThreadId, resolved: bool) -> ForgeResult<()> {
        let mut state = self.enter("resolve")?;
        state.writes.push(Write::Resolve { thread: thread.clone(), resolved });
        match state.pulls.values_mut().flat_map(|d| d.threads.iter_mut()).find(|t| t.id == *thread) {
            Some(t) => {
                t.resolved = resolved;
                Ok(())
            }
            None => Err(ForgeError::NotFound("that thread".into())),
        }
    }
}

/// A repository with the settings most have, for a pull request to belong to.
pub fn repository(reference: &RepoRef) -> Repository {
    Repository {
        id: "R_fixture".into(),
        reference: reference.clone(),
        url: format!("https://github.com/{}", reference.slug()),
        default_branch: Some("main".into()),
        merge: MergeSettings::default(),
        can_write: true,
    }
}

/// Plain pieces to build a pull request from, with every field a sensible value.
pub mod sample {
    use lathe_forge::{
        Author, CheckCounts, CheckStatus, Comment, Conclusion, MergeState, Pull, PullId, PullRef, PullState, Rights, Side, Thread,
        ThreadId,
    };

    use super::*;

    pub const NOW: u64 = 1_790_700_000;

    pub fn reference(number: u64) -> PullRef {
        PullRef { repo: RepoRef::new("github.com", "flazouh", "relay"), number }
    }

    pub fn pull(reference: &PullRef, head_sha: &str) -> Pull {
        Pull {
            id: PullId(format!("PR_{}", reference.number)),
            reference: reference.clone(),
            title: "Detach the byte stream before a second write".into(),
            body: "A client that aborted between two chunks left the relay writing into a closed sink.".into(),
            state: PullState::Open,
            url: format!("https://github.com/{}/pull/{}", reference.repo.slug(), reference.number),
            author: "Rui".into(),
            base: "main".into(),
            base_sha: String::new(),
            head: "rui/detach".into(),
            head_sha: head_sha.into(),
            created_at: NOW - 86_400,
            updated_at: NOW - 3_600,
            additions: 0,
            deletions: 0,
            changed_files: 0,
            remarks: 0,
            conflicting: false,
            merge_state: MergeState::Clean,
            review: ReviewDecision::Required,
            opinions: Vec::new(),
            requested: Vec::new(),
            checks: CheckCounts { passed: 1, failed: 0, running: 0 },
            queue: None,
            auto_merge: false,
            rights: Rights::Merge,
            can_update: true,
            merge: MergeSettings::default(),
        }
    }

    pub fn comment(id: &str, author: &str, body: &str, at: u64) -> Comment {
        Comment { id: id.into(), author: author.into(), kind: Author::Person, body: body.into(), created_at: at, unsent: false }
    }

    /// An open thread on `line` of `path`, the new side.
    pub fn thread(id: &str, path: &str, line: u32, comments: Vec<Comment>) -> Thread {
        Thread {
            id: ThreadId(id.into()),
            resolved: false,
            outdated: false,
            path: path.into(),
            file_level: false,
            line: Some(line),
            start_line: None,
            original_line: Some(line),
            side: Side::Right,
            can_resolve: true,
            can_reply: true,
            comments,
        }
    }

    pub fn check(name: &str, status: CheckStatus, conclusion: Option<Conclusion>) -> Check {
        Check { name: name.into(), status, conclusion, url: None, required: false, started_at: Some(NOW - 600), completed_at: conclusion.map(|_| NOW - 60), job: None, run: None }
    }

    /// A pull request with these files, all read.
    pub fn data(number: u64, head_sha: &str, files: Vec<ChangedFile>) -> PullData {
        let reference = reference(number);
        let mut data = PullData::new(reference.clone());
        let mut pull = pull(&reference, head_sha);
        pull.changed_files = files.len() as u32;
        pull.additions = files.iter().map(|f| f.additions).sum();
        pull.deletions = files.iter().map(|f| f.deletions).sum();
        data.apply(crate::Part::Pull(Box::new(pull)), NOW);
        data.apply(crate::Part::Files(files), NOW);
        data.apply(crate::Part::Threads(Vec::new()), NOW);
        data.apply(crate::Part::Remarks(Vec::new()), NOW);
        data.apply(crate::Part::Checks(Vec::new()), NOW);
        data.apply(crate::Part::ReviewPoint(None), NOW);
        data.apply(crate::Part::Held(Vec::new()), NOW);
        data
    }
}
