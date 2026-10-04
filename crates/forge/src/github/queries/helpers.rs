/// A query for the chip fields of many pull requests of one repository, each under its own alias: what a
/// list row knows, so a chip's card can say who, how big, and how it stands.
pub(in super::super) fn briefs(numbers: &[u64]) -> String {
    let fields = "number title state isDraft merged url updatedAt createdAt author { login } reviewDecision additions deletions \
        comments { totalCount } commits(last: 1) { nodes { commit { statusCheckRollup { state contexts(first: 1) { \
        checkRunCountsByState { state count } statusContextCountsByState { state count } } } } } }";
    let aliases: String = numbers.iter().map(|n| format!("p{n}: pullRequest(number: {n}) {{ {fields} }} ")).collect();
    format!("query Briefs($owner: String!, $name: String!) {{ repository(owner: $owner, name: $name) {{ nameWithOwner {aliases} }} }}")
}
