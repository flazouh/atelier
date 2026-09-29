//! The GraphQL queries, one file each in `queries/`.
pub(super) const REPOSITORY: &str = include_str!("queries/repository.graphql");
pub(super) const PULL: &str = include_str!("queries/pull.graphql");
pub(super) const FILES: &str = include_str!("queries/files.graphql");
pub(super) const THREADS: &str = include_str!("queries/threads.graphql");
pub(super) const THREAD_COMMENTS: &str = include_str!("queries/thread_comments.graphql");
pub(super) const CONVERSATION: &str = include_str!("queries/conversation.graphql");
pub(super) const CHECKS: &str = include_str!("queries/checks.graphql");
pub(super) const INVOLVED: &str = include_str!("queries/involved.graphql");
pub(super) const FOR_WRITE: &str = include_str!("queries/for_write.graphql");
pub(super) const PENDING_REVIEW: &str = include_str!("queries/pending_review.graphql");

/// A query for the chip fields of many pull requests of one repository, each under its own alias.
pub(super) fn briefs(numbers: &[u64]) -> String {
    let fields = "number title state isDraft merged url";
    let aliases: String = numbers.iter().map(|n| format!("p{n}: pullRequest(number: {n}) {{ {fields} }} ")).collect();
    format!("query Briefs($owner: String!, $name: String!) {{ repository(owner: $owner, name: $name) {{ nameWithOwner {aliases} }} }}")
}
