//! The GraphQL queries, one file each in `queries/`.

mod helpers;
mod types;

pub(super) use helpers::briefs;
pub(super) use types::{
    CHECKS, CONVERSATION, FILES, FOR_WRITE, INVOLVED, OPEN_PULL_FOR, PENDING_REVIEW, PULL,
    REPOSITORY, THREADS, THREAD_COMMENTS,
};
