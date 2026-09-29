//! The reader's working set and the batched lookup: many requests in flight at once, or one request
//! with many aliases.
use std::{collections::HashSet, thread};

use serde_json::{Value, json};

use super::{
    client::Client,
    queries, read,
    wire::{Page, SearchHit},
};
use crate::{ForgeError, ForgeResult, Involved, PullBrief, PullRef, PullState, RepoRef, Shelf};

/// The GitHub searches behind each shelf. `None` is a pull request the reader is only assigned to or
/// mentioned in: it belongs to no shelf.
const SEARCHES: &[(Option<Shelf>, &str)] = &[
    (Some(Shelf::NeedsAction), "is:pr is:open archived:false user-review-requested:@me"),
    (Some(Shelf::NeedsAction), "is:pr is:open archived:false author:@me review:changes_requested"),
    (Some(Shelf::NeedsAction), "is:pr is:open archived:false author:@me draft:false status:failure"),
    (Some(Shelf::TeamReviewRequested), "is:pr is:open archived:false team-review-requested:@me"),
    (Some(Shelf::WaitingForReview), "is:pr is:open archived:false author:@me draft:false -review:approved"),
    (Some(Shelf::ReadyToMerge), "is:pr is:open archived:false author:@me draft:false review:approved"),
    (Some(Shelf::YourDrafts), "is:pr is:open archived:false author:@me draft:true"),
    (Some(Shelf::MergeQueue), "is:pr is:open archived:false author:@me is:queued"),
    (None, "is:pr is:open archived:false assignee:@me"),
    (None, "is:pr is:open archived:false mentions:@me"),
];

/// Every search at once, since each is a round trip and the reader waits for the slowest.
pub(super) fn involved(client: &Client) -> ForgeResult<Vec<Involved>> {
    let searched: Vec<ForgeResult<Vec<Involved>>> = thread::scope(|scope| {
        let running: Vec<_> = SEARCHES
            .iter()
            .map(|(shelf, search)| scope.spawn(move || search_one(client, *shelf, search)))
            .collect();
        running
            .into_iter()
            .map(|handle| handle.join().unwrap_or_else(|_| Err(ForgeError::Unexpected("a search stopped".into()))))
            .collect()
    });
    let mut all = Vec::new();
    for result in searched {
        all.extend(result?);
    }
    Ok(all)
}

fn search_one(client: &Client, shelf: Option<Shelf>, search: &str) -> ForgeResult<Vec<Involved>> {
    client.pages(queries::INVOLVED, json!({"search": search}), |data| {
        let page: Page<SearchHit> = serde_json::from_value(data["search"].clone())
            .map_err(|e| ForgeError::Unexpected(format!("a search has an unexpected shape: {e}")))?;
        let next = page.page_info.next();
        let rows = page.nodes.iter().flatten().filter_map(read::summary).map(|summary| Involved { summary, shelf }).collect();
        Ok((rows, next))
    })
}

/// How many pull numbers one query carries.
const MOST_ALIASES: usize = 100;

/// The chip fields of many numbers of one repository. One request per hundred numbers. A number that is
/// not a pull request, or is not there, is `None`.
pub(super) fn briefs(client: &Client, repo: &RepoRef, numbers: &[u64]) -> ForgeResult<Vec<Option<PullBrief>>> {
    let mut unique: Vec<u64> = Vec::new();
    let mut seen = HashSet::new();
    for number in numbers {
        if seen.insert(*number) {
            unique.push(*number);
        }
    }
    let mut found = std::collections::HashMap::with_capacity(unique.len());
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
            found.insert(*number, brief(repo, *number, &repository[format!("p{number}")]));
        }
    }
    Ok(numbers.iter().map(|n| found.get(n).cloned().flatten()).collect())
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
