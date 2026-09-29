//! A pull request: its header, its summary in a list, and the changes a reader asks to make to it.
use super::{
    merge::{MergeSettings, QueuePlace, Rights},
    repository::RepoRef,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PullRef {
    pub repo: RepoRef,
    pub number: u64,
}

/// The forge's own id for a pull request, which writes need.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PullId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PullState {
    Open,
    Draft,
    Merged,
    Closed,
}

/// What the forge says about merging the pull request now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeState {
    Clean,
    /// A check that is not required fails.
    Unstable,
    /// A rule holds it: a required check, a required review.
    Blocked,
    /// The branch is behind its base.
    Behind,
    /// The branch conflicts with its base.
    Dirty,
    Draft,
    Unknown,
}

/// What the required review says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewDecision {
    /// No rule asks for a review.
    NotRequired,
    Required,
    Approved,
    ChangesRequested,
}

/// One reviewer's standing opinion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opinion {
    pub reviewer: String,
    pub verdict: super::conversation::Verdict,
}

/// Someone or some team asked to review.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reviewer {
    Person(String),
    Team(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CheckCounts {
    pub passed: u32,
    pub failed: u32,
    pub running: u32,
}

impl CheckCounts {
    /// The checks in one word, or `None` when there are none.
    pub fn state(self) -> Option<CheckState> {
        if self.failed > 0 {
            Some(CheckState::Failing)
        } else if self.running > 0 {
            Some(CheckState::Running)
        } else if self.passed > 0 {
            Some(CheckState::Passing)
        } else {
            None
        }
    }
}

/// From worst to best: one failure outweighs any number of passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckState {
    Failing,
    Running,
    Passing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pull {
    pub id: PullId,
    pub reference: PullRef,
    pub title: String,
    pub body: String,
    pub state: PullState,
    pub url: String,
    pub author: String,
    pub base: String,
    pub head: String,
    /// The commit the head branch stands on now.
    pub head_sha: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub additions: u32,
    pub deletions: u32,
    pub changed_files: u32,
    /// Remarks: comments on the pull request as a whole.
    pub remarks: u32,
    /// The branch and its base change the same lines.
    pub conflicting: bool,
    pub merge_state: MergeState,
    pub review: ReviewDecision,
    pub opinions: Vec<Opinion>,
    pub requested: Vec<Reviewer>,
    pub checks: CheckCounts,
    pub queue: Option<QueuePlace>,
    pub auto_merge: bool,
    pub rights: Rights,
    pub can_update: bool,
    pub merge: MergeSettings,
}

/// What a list and a chip need to know about a pull request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullBrief {
    pub reference: PullRef,
    pub title: String,
    pub state: PullState,
    pub url: String,
}

/// One row of a reader's working set.
#[derive(Clone, Debug, PartialEq)]
pub struct PullSummary {
    pub brief: PullBrief,
    pub author: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub additions: u32,
    pub deletions: u32,
    pub comments: u32,
    pub review: ReviewDecision,
    /// `None` until the checks are known.
    pub checks: Option<CheckCounts>,
}

/// Which of the reader's lists a pull request came from. Each is a question the forge answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shelf {
    NeedsAction,
    TeamReviewRequested,
    WaitingForReview,
    ReadyToMerge,
    YourDrafts,
    MergeQueue,
}

impl Shelf {
    pub const ALL: [Shelf; 6] = [
        Self::NeedsAction,
        Self::TeamReviewRequested,
        Self::WaitingForReview,
        Self::ReadyToMerge,
        Self::YourDrafts,
        Self::MergeQueue,
    ];
}

/// An involved pull request and the shelf that holds it. One the reader is only assigned to or
/// mentioned in is on none.
#[derive(Clone, Debug, PartialEq)]
pub struct Involved {
    pub summary: PullSummary,
    pub shelf: Option<Shelf>,
}

/// A changed file, as a list shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
    pub change: Change,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewPull {
    pub title: String,
    pub body: String,
    pub base: String,
    pub head: String,
    pub draft: bool,
}

/// What to change on a pull request. A field left `None` stays.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PullUpdate {
    pub title: Option<String>,
    pub body: Option<String>,
    pub base: Option<String>,
    /// `Some(true)` marks it ready for review; `Some(false)` makes it a draft.
    pub ready: Option<bool>,
    /// `Some(true)` closes it; `Some(false)` reopens it.
    pub closed: Option<bool>,
}
