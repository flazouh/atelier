#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BaseChoice {
    /// From the merge base: the whole pull request.
    Whole,
    /// From the reader's last review.
    LastReview,
    /// From this commit.
    Commit(String),
}
