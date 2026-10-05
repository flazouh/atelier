#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PullState {
    Open,
    Draft,
    Merged,
    Closed,
}

/// What the forge says about merging the pull request now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    #[default]
    Unknown,
}

/// What the required review says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReviewDecision {
    /// No rule asks for a review.
    NotRequired,
    Required,
    Approved,
    ChangesRequested,
}

/// Someone or some team asked to review.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Reviewer {
    Person(String),
    Team(String),
}

/// From worst to best: one failure outweighs any number of passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CheckState {
    Failing,
    Running,
    Passing,
}

/// Which of the reader's lists a pull request came from. Each is a question the forge answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

/// A changed file, as a list shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Change {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}
