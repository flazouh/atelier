//! The forge path's numbers, against the targets in `docs/performance.md`. Everything here runs on
//! answers held in memory, so it measures atelier's own work (building the request, reading the JSON,
//! mapping to the model) and not the network. Run in release, on the HP:
//!   cargo test -p atelier-forge --release --test perf -- --ignored --nocapture --test-threads=1
mod support;

use std::time::{Duration, Instant};

use atelier_forge::{
    Forge, Involved, PullBrief, PullRef, PullState, PullSummary, RepoRef, ReviewDecision, Shelf, file_courts,
    github::testing::Fixtures, present,
};
use serde_json::{Value, json};
use support::{bun, github, pull_ref, read};

const RUNS: usize = 20;

fn measure(mut run: impl FnMut()) -> (Duration, Duration) {
    let mut times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let start = Instant::now();
            run();
            start.elapsed()
        })
        .collect();
    times.sort();
    (times[RUNS / 2], times[(RUNS * 95).div_ceil(100) - 1])
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.
}

/// An answer for `numbers` of one repository, each a merged pull request with a 60-character title.
fn briefs_answer(numbers: &[u64]) -> String {
    let mut repository = serde_json::Map::new();
    repository.insert("nameWithOwner".into(), json!("oven-sh/bun"));
    for n in numbers {
        repository.insert(
            format!("p{n}"),
            json!({"number": n, "title": format!("bun_core: StringBuilder views accept a zero-capacity buffer {n}"),
                   "state": "MERGED", "isDraft": false, "merged": true, "url": format!("https://github.com/oven-sh/bun/pull/{n}")}),
        );
    }
    json!({"data": {"repository": repository}}).to_string()
}

#[test]
#[ignore = "a measurement, run in release"]
fn fifty_pull_numbers_are_one_request_and_map_in_under_a_millisecond() {
    let numbers: Vec<u64> = (44_100..44_150).collect();
    let fixtures = Fixtures::new().ok("Briefs", briefs_answer(&numbers));
    let forge = github(&fixtures);
    let (median, p95) = measure(|| {
        let briefs = forge.briefs(&bun(), &numbers).unwrap();
        assert_eq!(briefs.iter().flatten().count(), 50);
    });
    assert_eq!(fixtures.sent_for("Briefs").len(), RUNS, "one request for each lookup of 50 numbers");
    println!("lookup of 50 numbers, one request: median {:.3} ms, p95 {:.3} ms (our work only)", ms(median), ms(p95));
    assert!(median < Duration::from_millis(1));
}

/// A pull request of `files` files and `threads` threads of `per_thread` comments, as GitHub pages them
/// (100 files a page, 50 threads a page), built from the recorded thread.
fn big_pull(files: usize, threads: usize, per_thread: usize) -> Fixtures {
    let mut fixtures = Fixtures::new();
    for page in 0..files.div_ceil(100) {
        let count = (files - page * 100).min(100);
        let nodes: Vec<Value> = (0..count)
            .map(|i| json!({"path": format!("src/module_{}/file_{}.rs", page, i), "additions": 40, "deletions": 12, "changeType": "MODIFIED"}))
            .collect();
        let more = (page + 1) * 100 < files;
        let cursor = more.then(|| format!("f{page}"));
        fixtures = fixtures.ok("Files", json!({"data": {"repository": {"pullRequest": {"files": {"pageInfo": {"hasNextPage": more, "endCursor": cursor}, "nodes": nodes}}}}}).to_string());
    }
    let recorded = read("Threads.json");
    let comment = recorded["data"]["repository"]["pullRequest"]["reviewThreads"]["nodes"][2]["comments"]["nodes"][0].clone();
    for page in 0..threads.div_ceil(50) {
        let count = (threads - page * 50).min(50);
        let nodes: Vec<Value> = (0..count)
            .map(|i| {
                let comments: Vec<Value> = (0..per_thread).map(|c| {
                    let mut comment = comment.clone();
                    comment["id"] = json!(format!("C_{page}_{i}_{c}"));
                    comment
                }).collect();
                json!({"id": format!("T_{page}_{i}"), "isResolved": i % 2 == 0, "isOutdated": false, "path": format!("src/module_{}/file_{}.rs", page, i),
                       "line": 10 + i, "startLine": null, "originalLine": 10 + i, "subjectType": "LINE", "diffSide": "RIGHT",
                       "viewerCanResolve": true, "viewerCanReply": true,
                       "comments": {"pageInfo": {"hasNextPage": false, "endCursor": null}, "nodes": comments}})
            })
            .collect();
        let more = (page + 1) * 50 < threads;
        let cursor = more.then(|| format!("t{page}"));
        fixtures = fixtures.ok("Threads", json!({"data": {"repository": {"pullRequest": {"reviewThreads": {"pageInfo": {"hasNextPage": more, "endCursor": cursor}, "nodes": nodes}}}}}).to_string());
    }
    fixtures
}

#[test]
#[ignore = "a measurement, run in release"]
fn a_300_file_pull_with_5000_comments_maps_in_under_a_hundred_milliseconds() {
    let fixtures = big_pull(300, 500, 10);
    let megabytes = fixtures.bytes() as f64 / 1e6;
    let forge = github(&fixtures);
    let (mut files, mut comments) = (0, 0);
    let (median, p95) = measure(|| {
        files = forge.files(&pull_ref()).unwrap().len();
        comments = forge.threads(&pull_ref()).unwrap().iter().map(|t| t.comments.len()).sum::<usize>();
    });
    println!("300 files, 500 threads, {comments} comments, {megabytes:.1} MB of JSON: median {:.1} ms, p95 {:.1} ms", ms(median), ms(p95));
    assert_eq!((files, comments), (300, 5000));
    assert!(median < Duration::from_millis(100));
}

fn involved(n: u64) -> Involved {
    let shelf = Shelf::ALL[(n % 6) as usize];
    Involved {
        summary: PullSummary {
            brief: PullBrief {
                reference: PullRef { repo: RepoRef::new("github.com", "o", format!("r{}", n % 40)), number: n },
                title: format!("Pull request number {n}"),
                state: if n.is_multiple_of(17) { PullState::Merged } else { PullState::Open },
                url: format!("https://github.com/o/r/pull/{n}"),
            },
            author: "ada".into(),
            created_at: 1_790_000_000,
            updated_at: 1_790_000_000 + n * 97,
            additions: 10,
            deletions: 2,
            comments: 3,
            review: if n.is_multiple_of(3) { ReviewDecision::NotRequired } else { ReviewDecision::Required },
            checks: Some(atelier_forge::CheckCounts { passed: 5, failed: n.is_multiple_of(5) as u32, running: n.is_multiple_of(7) as u32 }),
            standing: Default::default(),
        },
        shelf: Some(shelf),
    }
}

#[test]
#[ignore = "a measurement, run in release"]
fn filing_500_pull_requests_into_courts_and_into_rows_takes_under_five_milliseconds() {
    let set: Vec<Involved> = (1..=500).map(involved).collect();
    let now = 1_790_100_000;
    let mut rows = 0;
    let (median, p95) = measure(|| {
        let filed = file_courts(set.clone(), |_| false);
        let items: Vec<_> = filed.iter().flat_map(|(_, r)| r).map(|f| present::court_item(f, now)).collect();
        rows = items.len();
    });
    println!("500 pull requests filed and mapped to {rows} rows: median {:.3} ms, p95 {:.3} ms", ms(median), ms(p95));
    assert_eq!(rows, 500);
    assert!(median < Duration::from_millis(5));
}
