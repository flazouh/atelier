//! Checks that every change atelier can send is shaped as GitHub's schema says, without sending any.
//! Each write runs against answers held in memory, and the input it built is compared with the fields of
//! its input type, which GitHub tells a read-only introspection query. A field GitHub renamed or dropped
//! fails here, before the first live write. Ignored by default (it needs `gh` and a network):
//!   cargo test -p atelier-forge --test schema -- --ignored --nocapture
use std::sync::Arc;

use atelier_forge::{
    Forge, MergeMethod, MergeRequest, NewLine, NewPull, PullRef, PullUpdate, RepoRef, Side, ThreadId, Verdict,
    github::{GhCli, GitHub, Request, Transport, testing::Fixtures},
};
use atelier_project::LocalProject;
use serde_json::{Value, json};

fn data(value: Value) -> String {
    json!({"data": value}).to_string()
}

/// Answers for every write, in the shapes GitHub documents.
fn answers(queue: bool) -> Fixtures {
    let target = data(json!({"repository": {"mergeQueue": if queue { json!({"id": "MQ"}) } else { Value::Null },
        "pullRequest": {"id": "PR_1", "headRefName": "feat", "headRefOid": "abc", "isCrossRepository": false}}}));
    let comment = json!({"id": "C", "body": "b", "createdAt": "2026-09-29T15:14:16Z", "author": {"login": "a", "__typename": "User"}, "state": "PENDING"});
    Fixtures::new()
        .ok("Repository", data(json!({"repository": {"id": "R", "nameWithOwner": "o/r"}})))
        .ok("CreatePull", data(json!({"createPullRequest": {"pullRequest": {"number": 1, "repository": {"nameWithOwner": "o/r"}}}})))
        .ok("ForWrite", target)
        .ok("UpdatePull", data(json!({})))
        .ok("Ready", data(json!({})))
        .ok("Draft", data(json!({})))
        .ok("Merge", data(json!({})))
        .ok("AutoMerge", data(json!({})))
        .ok("Enqueue", data(json!({})))
        .ok("DELETE-repos-o-r-git-refs-heads-feat", "")
        .ok("AddComment", data(json!({"addComment": {"commentEdge": {"node": comment}}})))
        .ok("PendingReview", data(json!({"repository": {"pullRequest": {"id": "PR_1", "reviews": {"nodes": []}}}})))
        .ok("AddReview", data(json!({"addPullRequestReview": {"pullRequestReview": {"id": "REV"}}})))
        .ok("Hold", data(json!({"addPullRequestReviewThread": {"thread": {"id": "T", "comments": {"nodes": [comment]}}}})))
        .ok("SubmitReview", data(json!({})))
        .ok("Reply", data(json!({"addPullRequestReviewThreadReply": {"comment": comment}})))
        .ok("Resolve", data(json!({})))
        .ok("Unresolve", data(json!({})))
}

/// Runs every write once.
fn run_all_writes(fixtures: &Fixtures, queue: bool) {
    let forge = GitHub::with_transport(fixtures.clone());
    let repo = RepoRef::new("github.com", "o", "r");
    let pull = PullRef { repo: repo.clone(), number: 1 };
    let request = |method, when_ready, delete_branch| MergeRequest {
        method, title: Some("t".into()), message: Some("m".into()), expected_head: Some("abc".into()), when_ready, delete_branch,
    };
    if !queue {
        forge.create_pull(&repo, &NewPull { title: "t".into(), body: "b".into(), base: "main".into(), head: "feat".into(), draft: true }).unwrap();
        let all = PullUpdate { title: Some("t".into()), body: Some("b".into()), base: Some("main".into()), ready: None, closed: Some(true) };
        forge.update_pull(&pull, &all).unwrap();
        forge.update_pull(&pull, &PullUpdate { ready: Some(true), ..PullUpdate::default() }).unwrap();
        forge.update_pull(&pull, &PullUpdate { ready: Some(false), ..PullUpdate::default() }).unwrap();
        forge.merge(&pull, &request(MergeMethod::Squash, false, true)).unwrap();
        forge.merge(&pull, &request(MergeMethod::Rebase, true, false)).unwrap();
        forge.comment(&pull, "b").unwrap();
        let line = NewLine { path: "a".into(), line: 2, start_line: Some(1), side: Side::Left, body: "b".into() };
        forge.hold_comment(&pull, &line).unwrap();
        for verdict in [Verdict::Comment, Verdict::Approve, Verdict::RequestChanges] {
            forge.submit_review(&pull, verdict, "b").unwrap();
        }
        forge.reply(&ThreadId("T".into()), "b").unwrap();
        forge.resolve(&ThreadId("T".into()), true).unwrap();
        forge.resolve(&ThreadId("T".into()), false).unwrap();
    } else {
        forge.merge(&pull, &request(MergeMethod::Merge, false, false)).unwrap();
    }
}

fn introspect(gh: &GhCli, query: &str) -> Value {
    let body = json!({"query": query, "variables": {}}).to_string();
    let reply = gh.send(&Request { method: "POST", path: "graphql".into(), body: Some(body) }).unwrap();
    assert_eq!(reply.status, 200, "{}", reply.body);
    serde_json::from_str::<Value>(&reply.body).unwrap()["data"].clone()
}

#[test]
#[ignore = "asks GitHub for its schema through the real gh"]
fn every_write_is_shaped_as_githubs_schema_says() {
    let folder = tempfile::tempdir().unwrap();
    let gh = GhCli::new(Arc::new(LocalProject::open(folder.path()).unwrap()));
    let mut checked = 0;
    for queue in [false, true] {
        let fixtures = answers(queue);
        run_all_writes(&fixtures, queue);
        for sent in fixtures.sent() {
            let body: Value = serde_json::from_str(sent.body.as_deref().unwrap_or("null")).unwrap_or(Value::Null);
            let Some(query) = body["query"].as_str().filter(|q| q.starts_with("mutation")) else { continue };
            // `mutation Merge($input: MergePullRequestInput!) { mergePullRequest(input: $input) { ... } }`
            let input_type = query.split("$input: ").nth(1).and_then(|s| s.split('!').next()).unwrap();
            let field = query.split('{').nth(1).unwrap().split('(').next().unwrap().trim();
            let types = introspect(&gh, &format!("{{ __type(name: \"{input_type}\") {{ inputFields {{ name }} }} }}"));
            let known: Vec<String> = types["__type"]["inputFields"].as_array().unwrap_or_else(|| panic!("no input type {input_type}")).iter().map(|f| f["name"].as_str().unwrap().to_string()).collect();
            for key in body["variables"]["input"].as_object().unwrap().keys() {
                assert!(known.contains(key), "{field}: GitHub's {input_type} has no `{key}` (it has {known:?})");
            }
            let mutation = introspect(&gh, "{ __type(name: \"Mutation\") { fields { name } } }");
            assert!(mutation["__type"]["fields"].as_array().unwrap().iter().any(|f| f["name"] == field), "no mutation `{field}`");
            checked += 1;
        }
    }
    println!("{checked} mutations checked against GitHub's schema, none sent");
    assert!(checked >= 14);
}
