//! The reader's working set: ten searches, each answered by its own hand-made page in the shape of
//! GitHub's search results (recorded answers would hold the private pull requests of whoever ran the
//! recording). Then the pure step after it: filing into Courts and mapping to the rows atelier-ui draws.
mod support;

use std::sync::Mutex;

use atelier_forge::{
    Court, Forge, ForgeError, PullState, Shelf, file_courts, present,
    github::{GitHub, Reply, Request, Transport, TransportError},
};
use serde_json::{Value, json};

/// Answers a search by matching its text; anything else is an empty page.
struct BySearch {
    pages: Vec<(&'static str, Vec<Value>)>,
    searched: Mutex<Vec<String>>,
}

impl BySearch {
    fn new(pages: Vec<(&'static str, Vec<Value>)>) -> Self {
        Self { pages, searched: Mutex::default() }
    }
}

impl Transport for BySearch {
    fn send(&self, request: &Request) -> Result<Reply, TransportError> {
        let body: Value = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();
        let search = body["variables"]["search"].as_str().unwrap().to_string();
        self.searched.lock().unwrap().push(search.clone());
        let hits = self.pages.iter().find(|(key, _)| search.contains(key)).map(|(_, hits)| hits.clone()).unwrap_or_default();
        let page = json!({"data": {"search": {"pageInfo": {"hasNextPage": false, "endCursor": null}, "nodes": hits}}});
        Ok(Reply { status: 200, headers: Vec::new(), body: page.to_string() })
    }
}

fn hit(number: u64, updated: &str, review: Value, checks: &[(&str, u32)]) -> Value {
    let counts: Vec<Value> = checks.iter().map(|(state, count)| json!({"state": state, "count": count})).collect();
    let rollup = if checks.is_empty() {
        Value::Null
    } else {
        json!({"state": "PENDING", "contexts": {"checkRunCountsByState": counts, "statusContextCountsByState": []}})
    };
    json!({
        "id": format!("PR_{number}"), "number": number, "title": format!("Pull {number}"), "state": "OPEN",
        "isDraft": false, "merged": false, "url": format!("https://github.com/o/r/pull/{number}"),
        "createdAt": "2026-09-01T00:00:00Z", "updatedAt": updated, "repository": {"nameWithOwner": "o/r"},
        "author": {"login": "ada"}, "reviewDecision": review, "additions": 10, "deletions": 2,
        "comments": {"totalCount": 3}, "viewerLatestReview": null,
        "commits": {"nodes": [{"commit": {"statusCheckRollup": rollup}}]},
    })
}

fn working_set() -> BySearch {
    let idle = hit(1, "2026-09-29T10:00:00Z", Value::Null, &[]);
    BySearch::new(vec![
        ("user-review-requested", vec![idle, json!({})]),
        ("team-review-requested", vec![hit(2, "2026-09-29T09:00:00Z", json!("REVIEW_REQUIRED"), &[])]),
        ("-review:approved", vec![hit(3, "2026-09-29T08:00:00Z", json!("REVIEW_REQUIRED"), &[("IN_PROGRESS", 2), ("SUCCESS", 5)])]),
        ("draft:false review:approved", vec![hit(4, "2026-09-29T07:00:00Z", Value::Null, &[("SUCCESS", 9)])]),
        ("draft:true", vec![hit(5, "2026-09-29T06:00:00Z", Value::Null, &[])]),
        ("is:queued", vec![hit(6, "2026-09-29T05:00:00Z", Value::Null, &[("PENDING", 1)])]),
        ("assignee:@me", vec![hit(7, "2026-09-29T04:00:00Z", Value::Null, &[])]),
        ("mentions:@me", vec![hit(3, "2026-09-29T08:00:00Z", json!("REVIEW_REQUIRED"), &[("IN_PROGRESS", 2), ("SUCCESS", 5)])]),
    ])
}

#[test]
fn ten_searches_run_and_each_row_carries_the_shelf_that_found_it() {
    let transport = working_set();
    let forge = GitHub::with_transport(transport);
    let involved = forge.involved().unwrap();
    let shelf = |n: u64| involved.iter().find(|i| i.summary.brief.reference.number == n).unwrap().shelf;
    assert_eq!(shelf(1), Some(Shelf::NeedsAction));
    assert_eq!(shelf(2), Some(Shelf::TeamReviewRequested));
    assert_eq!(shelf(3), Some(Shelf::WaitingForReview));
    assert_eq!(shelf(4), Some(Shelf::ReadyToMerge));
    assert_eq!(shelf(5), Some(Shelf::YourDrafts));
    assert_eq!(shelf(6), Some(Shelf::MergeQueue));
    assert_eq!(shelf(7), None, "assigned only");
    assert_eq!(involved.len(), 7, "the mention of #3 is not a second row, and a hit that is not a pull is skipped");
}

#[test]
fn every_search_is_a_read_of_open_pull_requests_of_the_reader() {
    let transport = std::sync::Arc::new(working_set());
    struct Shared(std::sync::Arc<BySearch>);
    impl Transport for Shared {
        fn send(&self, request: &Request) -> Result<Reply, TransportError> {
            self.0.send(request)
        }
    }
    GitHub::with_transport(Shared(transport.clone())).involved().unwrap();
    let searched = transport.searched.lock().unwrap();
    assert_eq!(searched.len(), 10);
    assert!(searched.iter().all(|s| s.starts_with("is:pr is:open archived:false ") && s.contains("@me")), "{searched:?}");
}

#[test]
fn a_summary_carries_what_a_row_shows() {
    let involved = GitHub::with_transport(working_set()).involved().unwrap();
    let three = involved.iter().find(|i| i.summary.brief.reference.number == 3).unwrap();
    let s = &three.summary;
    assert_eq!((s.brief.title.as_str(), s.brief.state, s.author.as_str()), ("Pull 3", PullState::Open, "ada"));
    assert_eq!((s.additions, s.deletions, s.comments), (10, 2, 3));
    assert_eq!(s.updated_at, 1_790_668_800);
    let counts = s.checks.unwrap();
    assert_eq!((counts.passed, counts.running, counts.failed), (5, 2, 0));
    assert!(involved.iter().find(|i| i.summary.brief.reference.number == 1).unwrap().summary.checks.is_none(), "no checks yet is not zero checks");
}

#[test]
fn the_working_set_files_into_courts_and_maps_to_rows() {
    let involved = GitHub::with_transport(working_set()).involved().unwrap();
    let filed = file_courts(involved, |_| false);
    let shape: Vec<(Court, Vec<u64>)> = filed
        .iter()
        .map(|(court, rows)| (*court, rows.iter().map(|r| r.involved.summary.brief.reference.number).collect()))
        .collect();
    assert_eq!(
        shape,
        [
            (Court::NeedsYou, vec![1, 2, 4, 5]),
            (Court::Waiting, vec![7]),
            (Court::Running, vec![3, 6]),
        ]
    );
    let now = 1_790_668_800 + 2 * 3600;
    let rows: Vec<_> = filed.iter().flat_map(|(_, rows)| rows).map(|f| present::court_item(f, now)).collect();
    let four = rows.iter().find(|r| r.pr.number == 4).unwrap();
    assert_eq!(four.why.as_ref(), "Ready to merge");
    assert_eq!(four.age.as_ref(), "3h ago");
    assert_eq!(four.pr.repo.as_ref(), "o/r");
    assert_eq!((four.added, four.removed, four.comments), (10, 2, 3));
    let three = rows.iter().find(|r| r.pr.number == 3).unwrap();
    assert_eq!((three.why.as_ref(), three.checks.passed, three.checks.running), ("Checks running", 5, 2));
    assert_eq!(rows.iter().find(|r| r.pr.number == 6).unwrap().why.as_ref(), "In the merge queue");
    assert_eq!(rows.iter().find(|r| r.pr.number == 7).unwrap().why.as_ref(), "Involved");
}

#[test]
fn one_search_that_fails_fails_the_working_set_and_says_why() {
    struct Failing;
    impl Transport for Failing {
        fn send(&self, _: &Request) -> Result<Reply, TransportError> {
            Err(TransportError::Offline)
        }
    }
    assert_eq!(GitHub::with_transport(Failing).involved().err().unwrap(), ForgeError::Offline);
}
