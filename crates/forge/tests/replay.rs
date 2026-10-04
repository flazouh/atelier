//! Every read, on the answers recorded from GitHub for oven-sh/bun#44169 (`tests/fixtures/github`,
//! recorded with `tests/live.rs`), and the same reads bent to reach the cases the one pull request does
//! not have: a draft, a conflict, a queue, several pages.
mod support;

use atelier_forge::{
    Author, Change, CheckState, CheckStatus, Conclusion, Forge, ForgeError, MergeMethod, MergeState, PullState,
    ReviewDecision, Reviewer, Rights, Side, Verdict, github::testing::Fixtures,
};
use serde_json::{Value, json};
use support::{bun, github, patched_pull, pull_ref, read, recorded};

#[test]
fn the_repository_says_how_it_lets_pull_requests_land() {
    let repository = github(&recorded()).repository("git@github.com:oven-sh/bun.git").unwrap();
    assert_eq!(repository.reference, bun());
    assert_eq!(repository.default_branch.as_deref(), Some("main"));
    assert_eq!(repository.merge.methods, [MergeMethod::Squash]);
    assert_eq!(repository.merge.default_method, MergeMethod::Squash);
    assert!(repository.merge.auto_merge_allowed && repository.merge.delete_branch_on_merge && !repository.merge.has_queue);
    assert!(!repository.can_write, "the reader only reads here");
    assert!(repository.id.starts_with("MDEw"));
}

#[test]
fn a_pull_is_read_into_the_glossarys_terms() {
    let pull = github(&recorded()).pull(&pull_ref()).unwrap();
    assert_eq!(pull.title, "bun_core: StringBuilder views accept a zero-capacity buffer");
    assert_eq!((pull.state, pull.author.as_str()), (PullState::Merged, "robobun"));
    assert_eq!((pull.base.as_str(), pull.head.as_str()), ("main", "robobun/6c1ab446/stringbuilder-zero-capacity"));
    assert_eq!(pull.head_sha, "d405137bce56a332bfa8e2d7802ac69007d11c57");
    assert_eq!((pull.additions, pull.deletions, pull.changed_files, pull.remarks), (164, 10, 4, 5));
    assert_eq!((pull.created_at, pull.updated_at), (1_790_624_627, 1_790_656_333));
    assert_eq!(pull.review, ReviewDecision::Required);
    assert_eq!(pull.opinions.iter().map(|o| (o.reviewer.as_str(), o.verdict)).collect::<Vec<_>>(), [("coderabbitai", Verdict::Comment), ("claude", Verdict::Comment)]);
    assert_eq!((pull.checks.passed, pull.checks.failed, pull.checks.running), (13, 0, 0));
    assert_eq!(pull.checks.state(), Some(CheckState::Passing));
    assert_eq!(pull.rights, Rights::Cannot);
    assert!(!pull.can_update && !pull.conflicting && pull.queue.is_none() && !pull.auto_merge);
    assert_eq!(pull.merge_state, MergeState::Unknown, "GitHub does not compute it for a merged pull");
    assert_eq!(pull.body.len(), 13_219, "the body arrives whole");
}

#[test]
fn the_variants_of_a_pull_the_recording_lacks_are_mapped_from_bent_answers() {
    let pull = |change: fn(&mut Value)| github(&patched_pull(change)).pull(&pull_ref()).unwrap();

    let draft = pull(|r| {
        r["pullRequest"]["merged"] = json!(false);
        r["pullRequest"]["state"] = json!("OPEN");
        r["pullRequest"]["isDraft"] = json!(true);
    });
    assert_eq!(draft.state, PullState::Draft);

    let closed = pull(|r| {
        r["pullRequest"]["merged"] = json!(false);
        r["pullRequest"]["state"] = json!("CLOSED");
    });
    assert_eq!(closed.state, PullState::Closed);

    let conflicting = pull(|r| {
        r["pullRequest"]["mergeable"] = json!("CONFLICTING");
        r["pullRequest"]["mergeStateStatus"] = json!("DIRTY");
    });
    assert!(conflicting.conflicting);
    assert_eq!(conflicting.merge_state, MergeState::Dirty);

    let behind = pull(|r| r["pullRequest"]["mergeStateStatus"] = json!("BEHIND"));
    assert_eq!(behind.merge_state, MergeState::Behind);

    let queued = pull(|r| {
        r["pullRequest"]["isInMergeQueue"] = json!(true);
        r["pullRequest"]["mergeQueueEntry"] = json!({"position": 2});
        r["mergeQueue"] = json!({"id": "MQ_1"});
    });
    assert_eq!(queued.queue.and_then(|q| q.position), Some(2));
    assert!(queued.merge.has_queue);

    let waiting = pull(|r| r["pullRequest"]["autoMergeRequest"] = json!({"enabledAt": "2026-09-29T15:14:16Z"}));
    assert!(waiting.auto_merge);

    let admin = pull(|r| {
        r["viewerPermission"] = json!("ADMIN");
        r["pullRequest"]["viewerCanMergeAsAdmin"] = json!(true);
        r["pullRequest"]["viewerCanUpdate"] = json!(true);
    });
    assert_eq!((admin.rights, admin.can_update), (Rights::Bypass, true));
    let writer = pull(|r| r["viewerPermission"] = json!("WRITE"));
    assert_eq!(writer.rights, Rights::Merge);

    let asked = pull(|r| {
        r["pullRequest"]["reviewDecision"] = json!("CHANGES_REQUESTED");
        r["pullRequest"]["reviewRequests"] = json!({"nodes": [
            {"requestedReviewer": {"__typename": "User", "login": "ada"}},
            {"requestedReviewer": {"__typename": "Team", "slug": "core"}},
            {"requestedReviewer": null}
        ]});
        r["pullRequest"]["latestReviews"] = json!({"nodes": [
            {"author": {"login": "grace"}, "state": "CHANGES_REQUESTED"},
            {"author": {"login": "old"}, "state": "DISMISSED"},
            {"author": null, "state": "APPROVED"}
        ]});
    });
    assert_eq!(asked.review, ReviewDecision::ChangesRequested);
    assert_eq!(asked.requested, [Reviewer::Person("ada".into()), Reviewer::Team("core".into())]);
    let who: Vec<_> = asked.opinions.iter().map(|o| (o.reviewer.as_str(), o.verdict)).collect();
    assert_eq!(who, [("grace", Verdict::RequestChanges), ("ghost", Verdict::Approve)], "a dismissed review is no opinion");

    let none = pull(|r| r["pullRequest"]["reviewDecision"] = Value::Null);
    assert_eq!(none.review, ReviewDecision::NotRequired);
}

#[test]
fn checks_are_counted_from_both_check_runs_and_commit_statuses() {
    let mixed = |r: &mut Value| {
        r["pullRequest"]["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"] = json!({
            "checkRunCountsByState": [{"state": "SUCCESS", "count": 5}, {"state": "SKIPPED", "count": 1}, {"state": "IN_PROGRESS", "count": 2}, {"state": "TIMED_OUT", "count": 1}],
            "statusContextCountsByState": [{"state": "PENDING", "count": 1}, {"state": "ERROR", "count": 1}, {"state": "SUCCESS", "count": 1}]
        });
    };
    let counts = github(&patched_pull(mixed)).pull(&pull_ref()).unwrap().checks;
    assert_eq!((counts.passed, counts.failed, counts.running), (7, 2, 3));
    assert_eq!(counts.state(), Some(CheckState::Failing), "one failure outweighs the rest");

    let no_rollup = github(&patched_pull(|r| r["pullRequest"]["commits"]["nodes"][0]["commit"]["statusCheckRollup"] = Value::Null))
        .pull(&pull_ref())
        .unwrap();
    assert_eq!(no_rollup.checks.state(), None);
}

#[test]
fn a_pull_that_is_not_there_is_not_found() {
    let gone = |change: fn(&mut Value)| github(&patched_pull(change)).pull(&pull_ref()).err().unwrap();
    assert_eq!(gone(|r| r["pullRequest"] = Value::Null), ForgeError::NotFound("oven-sh/bun#44169".into()));
    assert_eq!(gone(|r| *r = Value::Null), ForgeError::NotFound("oven-sh/bun".into()));
}

#[test]
fn the_last_review_point_is_the_commit_of_the_readers_latest_review() {
    assert_eq!(github(&recorded()).last_review_point(&pull_ref()).unwrap(), None);
    let reviewed = patched_pull(|r| r["pullRequest"]["viewerLatestReview"] = json!({"state": "APPROVED", "commit": {"oid": "abc"}}));
    assert_eq!(github(&reviewed).last_review_point(&pull_ref()).unwrap().as_deref(), Some("abc"));
    let no_commit = patched_pull(|r| r["pullRequest"]["viewerLatestReview"] = json!({"state": "COMMENTED", "commit": null}));
    assert_eq!(github(&no_commit).last_review_point(&pull_ref()).unwrap(), None);
}

#[test]
fn the_changed_files_are_listed_with_their_sizes() {
    let files = github(&recorded()).files(&pull_ref()).unwrap();
    let shape: Vec<_> = files.iter().map(|f| (f.path.as_str(), f.additions, f.deletions, f.change)).collect();
    assert_eq!(
        shape,
        [
            ("src/bun_core/string/StringBuilder.rs", 28, 2, Change::Modified),
            ("src/sql_jsc/postgres/protocol/error_response_jsc.rs", 1, 7, Change::Modified),
            ("test/cli/install/hosted-git-info/boundary-conditions.test.ts", 89, 0, Change::Modified),
            ("test/cli/install/hosted-git-info/from-url.test.ts", 46, 1, Change::Modified),
        ]
    );
}

#[test]
fn threads_arrive_with_their_comments_their_lines_and_their_state() {
    let threads = github(&recorded()).threads(&pull_ref()).unwrap();
    assert_eq!(threads.len(), 5);
    assert!(threads.iter().all(|t| t.resolved && t.side == Side::Right && !t.file_level));
    let outdated: Vec<_> = threads.iter().map(|t| (t.outdated, t.line)).collect();
    assert_eq!(outdated, [(true, None), (true, None), (false, Some(205)), (false, Some(116)), (false, Some(78))]);
    assert!(threads[0].original_line.is_some(), "an outdated thread still knows where it was written");
    assert_eq!(threads[0].comments.len(), 3);
    assert_eq!(threads[2].path, "src/bun_core/string/StringBuilder.rs");
    let first = &threads[2].comments[0];
    assert_eq!((first.author.as_str(), first.kind), ("claude", Author::Bot));
    assert!(first.body.starts_with("🟡 nit (optional)") && !first.unsent && first.created_at > 0);
}

#[test]
fn remarks_are_the_comments_on_the_whole_pull() {
    let remarks = github(&recorded()).remarks(&pull_ref()).unwrap();
    let who: Vec<_> = remarks.iter().map(|r| (r.author.as_str(), r.kind)).collect();
    assert_eq!(
        who,
        [("robobun", Author::Person), ("coderabbitai", Author::Bot), ("robobun", Author::Person), ("Jarred-Sumner", Author::Person), ("robobun", Author::Person)]
    );
}

#[test]
fn checks_carry_their_conclusion_their_rule_and_their_job() {
    let checks = github(&recorded()).checks(&pull_ref()).unwrap();
    assert_eq!(checks.len(), 13);
    let skipped = checks.iter().find(|c| c.name == "cancel").unwrap();
    assert_eq!((skipped.status, skipped.conclusion), (CheckStatus::Done, Some(Conclusion::Skipped)));
    let lint = checks.iter().find(|c| c.name == "Lint JavaScript").unwrap();
    assert_eq!(lint.run.as_ref().map(|r| r.workflow.as_str()), Some("Lint"));
    assert_eq!(lint.job.as_ref().map(|j| j.id), Some(109_198_139_454));
    assert!(lint.started_at.is_some() && lint.completed_at >= lint.started_at);
    let required: Vec<_> = checks.iter().filter(|c| c.required).map(|c| c.name.as_str()).collect();
    assert_eq!(required, ["Format", "buildkite/bun"]);
    let outside = checks.iter().find(|c| c.name == "buildkite/bun").unwrap();
    assert!(outside.job.is_none() && outside.run.is_none(), "another app's check has no steps to fetch");
    let neutral = checks.iter().find(|c| c.name == "Claude Code Review").unwrap();
    assert_eq!(neutral.conclusion, Some(Conclusion::Neutral));
}

#[test]
fn a_commit_status_reads_as_a_check_that_is_running_until_it_settles() {
    let status = |state: &str| {
        json!({"data": {"repository": {"pullRequest": {"commits": {"nodes": [{"commit": {"oid": "x", "statusCheckRollup": {"contexts": {
            "pageInfo": {"hasNextPage": false, "endCursor": null},
            "nodes": [{"__typename": "StatusContext", "context": "ci/legacy", "state": state, "targetUrl": "https://ci", "createdAt": "2026-09-29T15:14:16Z", "isRequired": true}]
        }}}}]}}}}})
        .to_string()
    };
    for (state, done, conclusion) in [("SUCCESS", CheckStatus::Done, Some(Conclusion::Success)), ("ERROR", CheckStatus::Done, Some(Conclusion::Failure)), ("PENDING", CheckStatus::Running, None)] {
        let checks = github(&Fixtures::new().ok("Checks", status(state))).checks(&pull_ref()).unwrap();
        assert_eq!((checks[0].status, checks[0].conclusion, checks[0].required), (done, conclusion, true), "{state}");
    }
}

#[test]
fn a_job_has_its_steps_and_an_attempt_and_its_log_is_fetched_only_when_asked() {
    let forge_fixtures = recorded();
    let forge = github(&forge_fixtures);
    let checks = forge.checks(&pull_ref()).unwrap();
    let job_ref = checks.iter().find(|c| c.name == "close-linked-issues").and_then(|c| c.job.clone()).unwrap();
    assert_eq!(forge_fixtures.sent().len(), 1, "listing the checks fetched no job and no log");
    let job = forge.job(&job_ref).unwrap();
    assert_eq!((job.name.as_str(), job.status, job.conclusion, job.attempt), ("close-linked-issues", CheckStatus::Done, Some(Conclusion::Success), 1));
    assert_eq!(job.steps.len(), 3);
    assert_eq!((job.steps[0].number, job.steps[0].name.as_str()), (1, "Set up job"));
    assert!(job.steps.iter().all(|s| s.conclusion == Some(Conclusion::Success) && s.started_at.is_some()));
    let log = forge.job_log(&job_ref).unwrap();
    assert!(log.starts_with("2026-09-29T04:32:18"), "the byte order mark is gone: {:?}", &log[..12]);
    assert!(log.len() > 30_000);
}

#[test]
fn briefs_come_back_in_order_from_one_request_and_a_non_pull_is_none() {
    let fixtures = recorded();
    let briefs = github(&fixtures).briefs(&bun(), &[44169, 1, 44032, 44169]).unwrap();
    assert_eq!(briefs.len(), 4);
    let first = briefs[0].as_ref().unwrap();
    assert_eq!((first.brief.reference.number, first.brief.state), (44169, PullState::Merged));
    assert_eq!(first.brief.url, "https://github.com/oven-sh/bun/pull/44169");
    assert!(briefs[1].is_none());
    assert_eq!(briefs[2].as_ref().unwrap().brief.title, "node:http: second listen() throws, close() reaps pre-request sockets");
    assert_eq!(briefs[3], briefs[0], "a number asked twice is answered twice");
    let sent = fixtures.sent_for("Briefs");
    assert_eq!(sent.len(), 1);
    let query = sent[0].body.as_deref().unwrap();
    assert_eq!(query.matches("pullRequest(number:").count(), 3, "each number once");
}

#[test]
fn a_brief_says_who_wrote_it_how_big_it_is_and_how_it_stands() {
    let node = json!({"number": 7, "title": "chore(ui): Faster chips", "state": "OPEN", "isDraft": false, "merged": false,
        "url": "https://github.com/oven-sh/bun/pull/7", "updatedAt": "2026-10-04T00:00:00Z", "author": {"login": "alex"},
        "reviewDecision": "APPROVED", "additions": 120, "deletions": 34, "comments": {"totalCount": 4},
        "commits": {"nodes": [{"commit": {"statusCheckRollup": {"state": "SUCCESS", "contexts": {
            "checkRunCountsByState": [{"state": "SUCCESS", "count": 12}], "statusContextCountsByState": []}}}}]}});
    let answer = json!({"data": {"repository": {"nameWithOwner": "oven-sh/bun", "p7": node}}});
    let fixtures = Fixtures::new().ok("Briefs", answer.to_string());
    let found = github(&fixtures).briefs(&bun(), &[7]).unwrap();
    let pull = found[0].as_ref().unwrap();
    assert_eq!((pull.author.as_str(), pull.additions, pull.deletions, pull.comments), ("alex", 120, 34, 4));
    assert_eq!(pull.review, ReviewDecision::Approved);
    assert_eq!(pull.checks.map(|c| c.passed), Some(12));
    assert!(pull.updated_at > 0);
    let query = fixtures.sent_for("Briefs")[0].body.clone().unwrap();
    for field in ["additions", "deletions", "author", "reviewDecision", "statusCheckRollup", "updatedAt"] {
        assert!(query.contains(field), "the lookup asks for {field}");
    }
}

#[test]
fn a_hundred_and_one_numbers_take_two_requests() {
    let repository = json!({"data": {"repository": {"nameWithOwner": "oven-sh/bun"}}});
    let fixtures = Fixtures::new().ok("Briefs", repository.to_string());
    let numbers: Vec<u64> = (1..=101).collect();
    let briefs = github(&fixtures).briefs(&bun(), &numbers).unwrap();
    assert_eq!(fixtures.sent_for("Briefs").len(), 2);
    assert!(briefs.iter().all(Option::is_none));
}

#[test]
fn a_repository_that_is_not_there_fails_the_lookup() {
    let fixtures = Fixtures::new().ok("Briefs", json!({"data": {"repository": null}, "errors": [{"type": "NOT_FOUND", "message": "gone"}]}).to_string());
    assert!(matches!(github(&fixtures).briefs(&bun(), &[1]), Err(ForgeError::NotFound(_))));
}

#[test]
fn an_error_other_than_not_found_fails_the_lookup() {
    let body = json!({"data": {"repository": {"p1": null}}, "errors": [{"type": "FORBIDDEN", "message": "no access"}]});
    let fixtures = Fixtures::new().ok("Briefs", body.to_string());
    assert_eq!(github(&fixtures).briefs(&bun(), &[1]).err().unwrap(), ForgeError::Denied("no access".into()));
}

/// The recorded thread page, with its threads copied `copies` times under new ids.
fn thread_page(copies: usize, next: Option<&str>) -> Value {
    let recorded = read("Threads.json");
    let template = recorded["data"]["repository"]["pullRequest"]["reviewThreads"]["nodes"].as_array().unwrap().clone();
    let nodes: Vec<Value> = (0..copies)
        .flat_map(|copy| {
            template.iter().enumerate().map(move |(i, node)| {
                let mut node = node.clone();
                node["id"] = json!(format!("T_{copy}_{i}"));
                node
            })
        })
        .collect();
    json!({"data": {"repository": {"pullRequest": {"reviewThreads": {"pageInfo": {"hasNextPage": next.is_some(), "endCursor": next}, "nodes": nodes}}}}})
}

#[test]
fn threads_follow_their_pages_to_the_end() {
    let fixtures = Fixtures::new()
        .ok("Threads", thread_page(2, Some("cursor-1")).to_string())
        .ok("Threads", thread_page(1, None).to_string());
    let threads = github(&fixtures).threads(&pull_ref()).unwrap();
    assert_eq!(threads.len(), 15);
    let sent = fixtures.sent_for("Threads");
    assert!(sent[1].body.as_deref().unwrap().contains("cursor-1"));
}

#[test]
fn a_thread_with_more_comments_than_a_page_fetches_the_rest() {
    let mut page = thread_page(1, None);
    let first = &mut page["data"]["repository"]["pullRequest"]["reviewThreads"]["nodes"][0]["comments"];
    let have = first["nodes"].as_array().unwrap().len();
    first["pageInfo"] = json!({"hasNextPage": true, "endCursor": "more"});
    let mut extra = first["nodes"][0].clone();
    extra["id"] = json!("C_EXTRA");
    let rest = json!({"data": {"node": {"comments": {"pageInfo": {"hasNextPage": false, "endCursor": null}, "nodes": [extra]}}}});
    let fixtures = Fixtures::new().ok("Threads", page.to_string()).ok("ThreadComments", rest.to_string());
    let threads = github(&fixtures).threads(&pull_ref()).unwrap();
    assert_eq!(threads[0].comments.len(), have + 1);
    assert_eq!(threads[0].comments.last().unwrap().id, "C_EXTRA");
    assert!(threads[1].comments.len() < have + 1);
    assert_eq!(fixtures.sent_for("ThreadComments").len(), 1);
}

#[test]
fn held_comments_are_the_unsent_ones_across_all_threads() {
    let mut page = thread_page(1, None);
    page["data"]["repository"]["pullRequest"]["reviewThreads"]["nodes"][2]["comments"]["nodes"][1]["state"] = json!("PENDING");
    let held = github(&Fixtures::new().ok("Threads", page.to_string())).held_comments(&pull_ref()).unwrap();
    assert_eq!(held.len(), 1);
    assert!(held[0].comment.unsent);
    assert_eq!(held[0].path, "src/bun_core/string/StringBuilder.rs");
    assert_eq!(held[0].line, Some(205));
}

#[test]
fn a_file_level_thread_is_told_from_a_line_thread() {
    let mut page = thread_page(1, None);
    let node = &mut page["data"]["repository"]["pullRequest"]["reviewThreads"]["nodes"][0];
    node["subjectType"] = json!("FILE");
    node["diffSide"] = json!("LEFT");
    let threads = github(&Fixtures::new().ok("Threads", page.to_string())).threads(&pull_ref()).unwrap();
    assert!(threads[0].file_level && threads[0].side == Side::Left);
    assert!(!threads[1].file_level);
}

#[test]
fn an_answer_of_the_wrong_shape_is_unexpected_and_names_the_shape() {
    let fixtures = Fixtures::new().ok("Files", json!({"data": {"repository": {"pullRequest": {"files": {"nodes": "x"}}}}}).to_string());
    assert!(matches!(github(&fixtures).files(&pull_ref()), Err(ForgeError::Unexpected(text)) if text.contains("shape")));
}

#[test]
fn checks_of_a_pull_with_no_commits_or_no_rollup_are_none() {
    let none = json!({"data": {"repository": {"pullRequest": {"commits": {"nodes": []}}}}});
    assert!(github(&Fixtures::new().ok("Checks", none.to_string())).checks(&pull_ref()).unwrap().is_empty());
    let no_rollup = json!({"data": {"repository": {"pullRequest": {"commits": {"nodes": [{"commit": {"oid": "x", "statusCheckRollup": null}}]}}}}});
    assert!(github(&Fixtures::new().ok("Checks", no_rollup.to_string())).checks(&pull_ref()).unwrap().is_empty());
}
