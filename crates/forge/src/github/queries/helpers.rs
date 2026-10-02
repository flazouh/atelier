/// A query for the chip fields of many pull requests of one repository, each under its own alias.
pub(in super::super) fn briefs(numbers: &[u64]) -> String {
    let fields = "number title state isDraft merged url";
    let aliases: String = numbers.iter().map(|n| format!("p{n}: pullRequest(number: {n}) {{ {fields} }} ")).collect();
    format!("query Briefs($owner: String!, $name: String!) {{ repository(owner: $owner, name: $name) {{ nameWithOwner {aliases} }} }}")
}
