use std::{collections::HashSet, thread};

use serde_json::json;

use super::super::{
    client::Client,
    queries,
    read,
    wire::{Page, SearchHit},
};
use crate::{ForgeError, ForgeResult, Involved, PullRef, RepoRef, Shelf};
use super::types::SEARCHES;

/// Every search at once, since each is a round trip and the reader waits for the slowest.
/// The searches for the whole working set, or with `repo:owner/name` on each for one repository.
pub(super) fn searches(scope: Option<&RepoRef>) -> Vec<(Option<Shelf>, String)> {
    SEARCHES
        .iter()
        .map(|(shelf, search)| match scope {
            Some(repo) => (*shelf, format!("{search} repo:{}", repo.slug())),
            None => (*shelf, search.to_string()),
        })
        .collect()
}

/// The reader's working set; with `scope`, only that repository's part of it.
pub(in super::super) fn involved(client: &Client, scope: Option<&RepoRef>) -> ForgeResult<Vec<Involved>> {
    let searches = searches(scope);
    let searched: Vec<ForgeResult<Vec<Involved>>> = thread::scope(|scope| {
        let running: Vec<_> = searches
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
    // A pull request the reader is only mentioned in is not a second row when a shelf holds it too.
    let shelved: HashSet<PullRef> =
        all.iter().filter(|row| row.shelf.is_some()).map(|row| row.summary.brief.reference.clone()).collect();
    all.retain(|row| row.shelf.is_some() || !shelved.contains(&row.summary.brief.reference));
    Ok(all)
}

fn search_one(client: &Client, shelf: Option<Shelf>, search: &str) -> ForgeResult<Vec<Involved>> {
    client.pages(queries::INVOLVED, json!({"search": search}), |mut data| {
        let page: Page<SearchHit> = serde_json::from_value(data["search"].take())
            .map_err(|e| ForgeError::Unexpected(format!("a search has an unexpected shape: {e}")))?;
        let next = page.page_info.next();
        let rows = page.nodes.iter().flatten().filter_map(read::summary).map(|summary| Involved { summary, shelf }).collect();
        Ok((rows, next))
    })
}
