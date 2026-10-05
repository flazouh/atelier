use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

use super::super::{client::Client, queries, read, wire::SearchHit};
use crate::{ForgeError, ForgeResult, PullBrief, PullRef, PullState, PullSummary, RepoRef};
use super::types::MOST_ALIASES;

/// The chip fields of many numbers of one repository. One request per hundred numbers. A number that is
/// not a pull request, or is not there, is `None`.
pub(in super::super) fn briefs(client: &Client, repo: &RepoRef, numbers: &[u64]) -> ForgeResult<Vec<Option<PullSummary>>> {
    let mut unique: Vec<u64> = Vec::new();
    let mut seen = HashSet::new();
    for number in numbers {
        if seen.insert(*number) {
            unique.push(*number);
        }
    }
    let mut found = HashMap::with_capacity(unique.len());
    for chunk in unique.chunks(MOST_ALIASES) {
        let vars = json!({"owner": repo.owner, "name": repo.name});
        let graph = client.graphql(&queries::briefs(chunk), vars)?;
        // A number that is an issue, or nothing, comes back null with a NOT_FOUND error at its alias:
        // that is an answer, not a failure. Any other error fails the call.
        if let Some(error) = graph.errors.into_iter().find(|e| e.kind.as_deref() != Some("NOT_FOUND")) {
            return Err(error.into());
        }
        let repository = &graph.data["repository"];
        if repository.is_null() {
            return Err(ForgeError::NotFound(repo.slug()));
        }
        for number in chunk {
            found.insert(*number, summary(repo, *number, &repository[format!("p{number}")]));
        }
    }
    Ok(numbers.iter().map(|n| found.get(n).cloned().flatten()).collect())
}

/// The open pull request whose head is `head`, if there is one.
pub(in super::super) fn open_for(client: &Client, repo: &RepoRef, head: &str) -> ForgeResult<Option<PullBrief>> {
    let vars = json!({"owner": repo.owner, "name": repo.name, "head": head});
    let graph = client.graphql(queries::OPEN_PULL_FOR, vars)?;
    if let Some(error) = graph.errors.into_iter().next() {
        return Err(error.into());
    }
    let repository = &graph.data["repository"];
    if repository.is_null() {
        return Err(ForgeError::NotFound(repo.slug()));
    }
    let Some(node) = repository["pullRequests"]["nodes"].as_array().and_then(|nodes| nodes.first()) else { return Ok(None) };
    let number = node["number"].as_u64().ok_or_else(|| ForgeError::Unexpected("a pull request with no number".into()))?;
    Ok(brief(repo, number, node))
}

fn summary(repo: &RepoRef, number: u64, node: &Value) -> Option<PullSummary> {
    let hit: SearchHit = serde_json::from_value(node.clone()).ok()?;
    Some(read::summary_of(brief(repo, number, node)?, &hit))
}

fn brief(repo: &RepoRef, number: u64, node: &Value) -> Option<PullBrief> {
    let text = |key: &str| node.get(key)?.as_str().map(str::to_string);
    let flag = |key: &str| node.get(key).and_then(Value::as_bool).unwrap_or(false);
    let state = match (text("state")?.as_str(), flag("merged"), flag("isDraft")) {
        (_, true, _) | ("MERGED", ..) => PullState::Merged,
        ("CLOSED", ..) => PullState::Closed,
        (_, _, true) => PullState::Draft,
        _ => PullState::Open,
    };
    Some(PullBrief { reference: PullRef { repo: repo.clone(), number }, title: text("title")?, state, url: text("url")? })
}
