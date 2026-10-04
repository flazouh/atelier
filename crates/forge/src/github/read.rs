//! GitHub's data as atelier's model. Pure: nothing here asks GitHub anything, so it is tested on recorded
//! answers.

mod helpers;
mod types;

pub(super) use helpers::{
    check, comment, file, job, last_review_point, pull, repo_ref, repository, summary, summary_of, thread,
};
pub(super) use types::HOST;
