use atelier_forge::{MergeRequest, UpdateMethod};

/// What a press in the merge box asks of the forge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// Merge now, when ready, or into the queue.
    Merge(MergeRequest),
    /// Take the pull request out of draft.
    Ready,
    /// Bring the base into the branch. `expected_head` is the commit the reader saw.
    UpdateBranch { method: UpdateMethod, expected_head: String },
    CancelAutoMerge,
    Dequeue,
    /// Delete the branch alone, after the pull request merged.
    DeleteBranch,
    /// Open a pull request that reverts this one.
    Revert,
}
