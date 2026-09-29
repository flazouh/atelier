//! The reader's working set: ten searches in flight at once.
use std::{collections::HashSet, thread};

use serde_json::json;

use super::{
    client::Client,
    queries, read,
    wire::{Page, SearchHit},
};
use crate::{ForgeError, ForgeResult, Involved, PullRef, Shelf};

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
