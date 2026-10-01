pub(in super::super) const REPOSITORY: &str = include_str!("../queries/repository.graphql");

pub(in super::super) const PULL: &str = include_str!("../queries/pull.graphql");

pub(in super::super) const FILES: &str = include_str!("../queries/files.graphql");

pub(in super::super) const THREADS: &str = include_str!("../queries/threads.graphql");

pub(in super::super) const THREAD_COMMENTS: &str = include_str!("../queries/thread_comments.graphql");

pub(in super::super) const CONVERSATION: &str = include_str!("../queries/conversation.graphql");

pub(in super::super) const CHECKS: &str = include_str!("../queries/checks.graphql");

pub(in super::super) const INVOLVED: &str = include_str!("../queries/involved.graphql");

pub(in super::super) const FOR_WRITE: &str = include_str!("../queries/for_write.graphql");

pub(in super::super) const PENDING_REVIEW: &str = include_str!("../queries/pending_review.graphql");

/// A query for the chip fields of many pull requests of one repository, each under its own alias.
/// The open pull request, if any, whose head is `$head`.
pub(in super::super) const OPEN_PULL_FOR: &str = "query OpenPullFor($owner: String!, $name: String!, $head: String!) { repository(owner: $owner, name: $name) { pullRequests(headRefName: $head, states: [OPEN], first: 1) { nodes { number title state isDraft merged url } } } }";
