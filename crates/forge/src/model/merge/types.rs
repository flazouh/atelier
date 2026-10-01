#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

/// The reader's right to merge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Rights {
    Merge,
    /// An administrator, who may merge past the rules that failed.
    Bypass,
    Cannot,
}

/// How "Update branch" brings the base into the branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UpdateMethod {
    /// A merge commit of the base into the branch.
    Merge,
    /// The branch's commits replayed on the base.
    Rebase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MergeOutcome {
    Merged,
    /// Set to merge itself when ready.
    WillMergeWhenReady,
    /// Put in the merge queue.
    Queued,
}
