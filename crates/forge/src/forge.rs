//! The interface to a forge. Every call blocks and may be slow, so none is made on the UI thread. Reads
//! that a screen can wait for in parts are separate calls: the header first, then the files, the threads
//! and the checks, and a job's log only when a reader opens it.
use crate::{
    ChangedFile, Check, Comment, ForgeResult, HeldComment, Involved, Job, JobRef, MergeOutcome, MergeRequest,
    NewLine, NewPull, Pull, PullBrief, PullRef, PullUpdate, RepoRef, Repository, Remark, Reviewer, Thread, ThreadId,
    Verdict,
};

pub trait Forge: Send + Sync {
    /// The repository a git remote URL names.
    fn repository(&self, remote_url: &str) -> ForgeResult<Repository>;

    /// One pull request: its header, its review standing, its checks in counts and what merging it
    /// needs. Not its files, threads or check list.
    fn pull(&self, reference: &PullRef) -> ForgeResult<Pull>;

    fn files(&self, reference: &PullRef) -> ForgeResult<Vec<ChangedFile>>;

    /// Review threads with all their comments, resolved ones included.
    fn threads(&self, reference: &PullRef) -> ForgeResult<Vec<Thread>>;

    /// Remarks: comments on the pull request as a whole.
    fn remarks(&self, reference: &PullRef) -> ForgeResult<Vec<Remark>>;

    /// The checks on the pull request's head, each with its job when it has one.
    fn checks(&self, reference: &PullRef) -> ForgeResult<Vec<Check>>;

    /// One job with its steps and its attempt.
    fn job(&self, job: &JobRef) -> ForgeResult<Job>;

    /// A job's log as text. Fetched only when asked for: a log can be megabytes.
    fn job_log(&self, job: &JobRef) -> ForgeResult<String>;

    /// The commit the reader last reviewed the pull request up to, when the forge keeps it.
    fn last_review_point(&self, reference: &PullRef) -> ForgeResult<Option<String>>;

    /// The pull requests the reader is involved in, each on the shelf that holds it.
    fn involved(&self) -> ForgeResult<Vec<Involved>>;

    /// Many pull numbers of one repository in one request. The answer has one entry per number, in
    /// order; a number that is not a pull request is `None`.
    fn briefs(&self, repo: &RepoRef, numbers: &[u64]) -> ForgeResult<Vec<Option<PullBrief>>>;

    fn create_pull(&self, repo: &RepoRef, new: &NewPull) -> ForgeResult<PullRef>;

    fn update_pull(&self, reference: &PullRef, update: &PullUpdate) -> ForgeResult<()>;

    fn merge(&self, reference: &PullRef, request: &MergeRequest) -> ForgeResult<MergeOutcome>;

    fn request_review(&self, reference: &PullRef, reviewers: &[Reviewer]) -> ForgeResult<()>;

    /// A remark on the pull request as a whole.
    fn comment(&self, reference: &PullRef, body: &str) -> ForgeResult<Comment>;

    /// Writes a comment on a line into the reader's review, held: nobody else sees it until
    /// `submit_review`.
    fn hold_comment(&self, reference: &PullRef, comment: &NewLine) -> ForgeResult<HeldComment>;

    /// The comments the forge holds for the reader on this pull request.
    fn held_comments(&self, reference: &PullRef) -> ForgeResult<Vec<HeldComment>>;

    /// Sends the reader's review with its held comments, and its verdict.
    fn submit_review(&self, reference: &PullRef, verdict: Verdict, body: &str) -> ForgeResult<()>;

    fn reply(&self, thread: &ThreadId, body: &str) -> ForgeResult<Comment>;

    fn resolve(&self, thread: &ThreadId, resolved: bool) -> ForgeResult<()>;
}
