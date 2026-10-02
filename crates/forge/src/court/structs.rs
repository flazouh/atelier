use crate::{CheckState, Involved, PullState, ReviewDecision, Shelf};
use super::types::Court;

/// What decides a Court.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weighing {
    /// `None` for a pull request on none of the reader's shelves.
    pub shelf: Option<Shelf>,
    pub state: PullState,
    /// The pull request stands on another that has not landed.
    pub stands_on_unlanded: bool,
    /// `None` until the checks are known.
    pub checks: Option<CheckState>,
    pub review: ReviewDecision,
}

/// A pull request with its Court.
#[derive(Clone, Debug, PartialEq)]
pub struct Filed {
    pub involved: Involved,
    pub court: Court,
}
