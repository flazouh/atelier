//! Merging: the repository's rules, what the reader may do, and the request and its outcome.

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

/// How a repository lets pull requests land.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MergeSettings {
    pub methods: Vec<MergeMethod>,
    pub default_method: MergeMethod,
    pub auto_merge_allowed: bool,
    pub delete_branch_on_merge: bool,
    pub has_queue: bool,
}

impl Default for MergeSettings {
    fn default() -> Self {
        Self {
            methods: vec![MergeMethod::Merge, MergeMethod::Squash, MergeMethod::Rebase],
            default_method: MergeMethod::Merge,
            auto_merge_allowed: false,
            delete_branch_on_merge: false,
            has_queue: false,
        }
    }
}

/// A pull request's place in a merge queue. The first in line is 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QueuePlace {
    pub position: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MergeRequest {
    pub method: MergeMethod,
    /// The commit's title and message; the forge's own when `None`.
    pub title: Option<String>,
    pub message: Option<String>,
    /// The head the reader saw. The forge refuses the merge when the branch has moved since.
    pub expected_head: Option<String>,
    /// Merge when the blockers that can wait clear, instead of now.
    pub when_ready: bool,
    pub delete_branch: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MergeOutcome {
    Merged,
    /// Set to merge itself when ready.
    WillMergeWhenReady,
    /// Put in the merge queue.
    Queued,
}
