use super::super::{
    merge::{MergeSettings, QueuePlace, Rights},
    repository::RepoRef,
};
use super::types::{Change, CheckState, MergeState, PullState, ReviewDecision, Reviewer, Shelf};

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PullRef {
    pub repo: RepoRef,
    pub number: u64,
}

/// The forge's own id for a pull request, which writes need.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PullId(pub String);

/// One reviewer's standing opinion.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Opinion {
    pub reviewer: String,
    pub verdict: super::super::conversation::Verdict,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Pull {
    pub id: PullId,
    pub reference: PullRef,
    pub title: String,
    pub body: String,
    pub state: PullState,
    pub url: String,
    pub author: String,
    pub base: String,
    /// The commit of the base branch the pull request stands on: for a merged one, the base before the
    /// merge. Empty when the forge did not say. The changes are those since the merge base of this and the head.
    pub base_sha: String,
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
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PullBrief {
    pub reference: PullRef,
    pub title: String,
    pub state: PullState,
    pub url: String,
}

/// One row of a reader's working set.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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

/// An involved pull request and the shelf that holds it. One the reader is only assigned to or
/// mentioned in is on none.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Involved {
    pub summary: PullSummary,
    pub shelf: Option<Shelf>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChangedFile {
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
    pub change: Change,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewPull {
    pub title: String,
    pub body: String,
    pub base: String,
    pub head: String,
    pub draft: bool,
}

/// What to change on a pull request. A field left `None` stays.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PullUpdate {
    pub title: Option<String>,
    pub body: Option<String>,
    pub base: Option<String>,
    /// `Some(true)` marks it ready for review; `Some(false)` makes it a draft.
    pub ready: Option<bool>,
    /// `Some(true)` closes it; `Some(false)` reopens it.
    pub closed: Option<bool>,
}
