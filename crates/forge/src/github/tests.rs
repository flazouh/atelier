//! The write paths and the error paths, on hand-made answers in GitHub's documented shapes. Nothing
//! here reaches GitHub: no test in this crate creates, comments, reviews, merges or pushes.
use serde_json::{Value, json};

use super::{GitHub, client::Client, testing::Fixtures, transport::TransportError};
use crate::{
    Forge, ForgeError, MergeMethod, MergeOutcome, MergeRequest, NewLine, NewPull, PullRef, PullUpdate, RepoRef,
    Reviewer, Side, ThreadId, Verdict,
};

fn github(fixtures: &Fixtures) -> GitHub {
    GitHub::with_client(Client::new(fixtures.clone()).with_clock(|_| {}, || 1000))
}

fn pull() -> PullRef {
    PullRef { repo: RepoRef::new("github.com", "o", "r"), number: 7 }
}

fn data(value: Value) -> String {
    json!({"data": value}).to_string()
}

fn for_write(queue: bool, fork: bool) -> String {
    data(json!({"repository": {
        "mergeQueue": if queue { json!({"id": "MQ"}) } else { Value::Null },
        "pullRequest": {"id": "PR_1", "headRefName": "feat", "headRefOid": "abc123", "isCrossRepository": fork},
    }}))
}

/// The `input` of the one request sent for `key`.
fn input(fixtures: &Fixtures, key: &str) -> Value {
    let sent = fixtures.sent_for(key);
    assert_eq!(sent.len(), 1, "{key} was sent {} times", sent.len());
    let body: Value = serde_json::from_str(sent[0].body.as_deref().unwrap()).unwrap();
    body["variables"]["input"].clone()
}

fn merge_request(method: MergeMethod) -> MergeRequest {
    MergeRequest { method, title: None, message: None, expected_head: None, when_ready: false, delete_branch: false }
}

#[test]
fn creating_a_pull_looks_up_the_repository_and_sends_the_new_pull() {
    let fixtures = Fixtures::new()
        .ok("Repository", data(json!({"repository": {"id": "R_1", "nameWithOwner": "o/r"}})))
        .ok("CreatePull", data(json!({"createPullRequest": {"pullRequest": {"number": 12, "repository": {"nameWithOwner": "o/r"}}}})));
    let new = NewPull { title: "Add x".into(), body: "Why".into(), base: "main".into(), head: "feat".into(), draft: true };
    let made = github(&fixtures).create_pull(&pull().repo, &new).unwrap();
    assert_eq!((made.number, made.repo.slug()), (12, "o/r".to_string()));
    assert_eq!(
        input(&fixtures, "CreatePull"),
        json!({"repositoryId": "R_1", "baseRefName": "main", "headRefName": "feat", "title": "Add x", "body": "Why", "draft": true})
    );
}

#[test]
fn updating_sends_only_the_fields_that_change() {
    let fixtures = Fixtures::new().ok("ForWrite", for_write(false, false)).ok("UpdatePull", data(json!({})));
    let update = PullUpdate { title: Some("New".into()), closed: Some(true), ..PullUpdate::default() };
    github(&fixtures).update_pull(&pull(), &update).unwrap();
    assert_eq!(input(&fixtures, "UpdatePull"), json!({"pullRequestId": "PR_1", "title": "New", "state": "CLOSED"}));
}

#[test]
fn marking_ready_and_back_to_draft_use_their_own_mutations_and_no_update() {
    let fixtures = Fixtures::new().ok("ForWrite", for_write(false, false)).ok("Ready", data(json!({}))).ok("Draft", data(json!({})));
    let forge = github(&fixtures);
    forge.update_pull(&pull(), &PullUpdate { ready: Some(true), ..PullUpdate::default() }).unwrap();
    forge.update_pull(&pull(), &PullUpdate { ready: Some(false), ..PullUpdate::default() }).unwrap();
    assert_eq!(input(&fixtures, "Ready"), json!({"pullRequestId": "PR_1"}));
    assert_eq!(input(&fixtures, "Draft"), json!({"pullRequestId": "PR_1"}));
    assert!(fixtures.sent_for("UpdatePull").is_empty());
}

#[test]
fn a_merge_names_the_method_and_the_head_the_reader_saw() {
    let fixtures = Fixtures::new().ok("ForWrite", for_write(false, false)).ok("Merge", data(json!({})));
    let request = MergeRequest {
        title: Some("T".into()),
        message: Some("M".into()),
        expected_head: Some("def456".into()),
        ..merge_request(MergeMethod::Squash)
    };
    assert_eq!(github(&fixtures).merge(&pull(), &request).unwrap(), MergeOutcome::Merged);
    assert_eq!(
        input(&fixtures, "Merge"),
        json!({"pullRequestId": "PR_1", "mergeMethod": "SQUASH", "commitHeadline": "T", "commitBody": "M", "expectedHeadOid": "def456"})
    );
}

#[test]
fn a_merge_with_no_expected_head_uses_the_head_it_just_read() {
    let fixtures = Fixtures::new().ok("ForWrite", for_write(false, false)).ok("Merge", data(json!({})));
    github(&fixtures).merge(&pull(), &merge_request(MergeMethod::Rebase)).unwrap();
    let sent = input(&fixtures, "Merge");
    assert_eq!((sent["mergeMethod"].as_str(), sent["expectedHeadOid"].as_str()), (Some("REBASE"), Some("abc123")));
}

#[test]
fn the_branch_is_deleted_after_the_merge_lands_and_only_when_asked() {
    let fixtures = Fixtures::new()
        .ok("ForWrite", for_write(false, false))
        .ok("Merge", data(json!({})))
        .ok("DELETE-repos-o-r-git-refs-heads-feat", "");
    let request = MergeRequest { delete_branch: true, ..merge_request(MergeMethod::Merge) };
    github(&fixtures).merge(&pull(), &request).unwrap();
    assert_eq!(fixtures.sent_for("DELETE-repos-o-r-git-refs-heads-feat").len(), 1);
    let order: Vec<_> = fixtures.sent().iter().map(super::testing::key_of).collect();
    assert_eq!(order, ["ForWrite", "Merge", "DELETE-repos-o-r-git-refs-heads-feat"], "delete comes after the merge");
}

#[test]
fn a_branch_in_a_fork_is_never_deleted() {
    let fixtures = Fixtures::new().ok("ForWrite", for_write(false, true)).ok("Merge", data(json!({})));
    let request = MergeRequest { delete_branch: true, ..merge_request(MergeMethod::Merge) };
    github(&fixtures).merge(&pull(), &request).unwrap();
    assert_eq!(fixtures.sent().len(), 2);
}

#[test]
fn a_refused_merge_says_why_in_the_forges_words_and_deletes_nothing() {
    let refused = json!({"data": null, "errors": [{"type": "UNPROCESSABLE", "message": "Pull Request is not mergeable"}]}).to_string();
    let fixtures = Fixtures::new().ok("ForWrite", for_write(false, false)).ok("Merge", refused);
    let request = MergeRequest { delete_branch: true, ..merge_request(MergeMethod::Merge) };
    let error = github(&fixtures).merge(&pull(), &request).err().unwrap();
    assert_eq!(error, ForgeError::Rejected("Pull Request is not mergeable".into()));
    assert_eq!(fixtures.sent().len(), 2);
}

#[test]
fn merge_when_ready_turns_on_auto_merge() {
    let fixtures = Fixtures::new().ok("ForWrite", for_write(false, false)).ok("AutoMerge", data(json!({})));
    let request = MergeRequest { when_ready: true, ..merge_request(MergeMethod::Squash) };
    assert_eq!(github(&fixtures).merge(&pull(), &request).unwrap(), MergeOutcome::WillMergeWhenReady);
    assert_eq!(input(&fixtures, "AutoMerge")["mergeMethod"], "SQUASH");
    assert!(fixtures.sent_for("Merge").is_empty());
}

#[test]
fn a_repository_with_a_queue_takes_a_merge_into_the_queue() {
    let fixtures = Fixtures::new().ok("ForWrite", for_write(true, false)).ok("Enqueue", data(json!({})));
    assert_eq!(github(&fixtures).merge(&pull(), &merge_request(MergeMethod::Merge)).unwrap(), MergeOutcome::Queued);
    assert_eq!(input(&fixtures, "Enqueue")["pullRequestId"], "PR_1");
    assert!(fixtures.sent_for("Merge").is_empty());
}

#[test]
fn a_pull_that_does_not_exist_is_not_found_before_anything_is_changed() {
    let none = data(json!({"repository": {"mergeQueue": null, "pullRequest": null}}));
    let fixtures = Fixtures::new().ok("ForWrite", none);
    let error = github(&fixtures).merge(&pull(), &merge_request(MergeMethod::Merge)).err().unwrap();
    assert_eq!(error, ForgeError::NotFound("o/r#7".into()));
    assert_eq!(fixtures.sent().len(), 1);
}

#[test]
fn asking_for_review_sends_people_and_teams_apart() {
    let fixtures = Fixtures::new().ok("POST-repos-o-r-pulls-7-requested-reviewers", "{}");
    let who = [Reviewer::Person("ada".into()), Reviewer::Team("core".into()), Reviewer::Person("grace".into())];
    github(&fixtures).request_review(&pull(), &who).unwrap();
    let sent = fixtures.sent();
    let body: Value = serde_json::from_str(sent[0].body.as_deref().unwrap()).unwrap();
    assert_eq!(body, json!({"reviewers": ["ada", "grace"], "team_reviewers": ["core"]}));
}

#[test]
fn a_remark_comes_back_as_the_comment_that_was_made() {
    let node = json!({"id": "IC_1", "body": "Thanks", "createdAt": "2026-09-29T15:14:16Z", "author": {"login": "ada", "__typename": "User"}});
    let fixtures = Fixtures::new()
        .ok("ForWrite", for_write(false, false))
        .ok("AddComment", data(json!({"addComment": {"commentEdge": {"node": node}}})));
    let comment = github(&fixtures).comment(&pull(), "Thanks").unwrap();
    assert_eq!((comment.author.as_str(), comment.body.as_str(), comment.created_at), ("ada", "Thanks", 1_790_694_856));
    assert_eq!(input(&fixtures, "AddComment"), json!({"subjectId": "PR_1", "body": "Thanks"}));
}

fn pending(existing: Option<&str>) -> String {
    let nodes = existing.map_or(json!([]), |id| json!([{"id": id}]));
    data(json!({"repository": {"pullRequest": {"id": "PR_1", "reviews": {"nodes": nodes}}}}))
}

fn held_thread(state: &str) -> String {
    let comment = json!({"id": "C_1", "body": "nit", "createdAt": "2026-09-29T15:14:16Z", "author": {"login": "me", "__typename": "User"}, "state": state});
    data(json!({"addPullRequestReviewThread": {"thread": {"id": "T_1", "comments": {"nodes": [comment]}}}}))
}

#[test]
fn a_held_comment_starts_a_review_when_there_is_none_and_is_marked_unsent() {
    let fixtures = Fixtures::new()
        .ok("PendingReview", pending(None))
        .ok("AddReview", data(json!({"addPullRequestReview": {"pullRequestReview": {"id": "REV_1"}}})))
        .ok("Hold", held_thread("PENDING"));
    let line = NewLine { path: "src/a.rs".into(), line: 10, start_line: Some(8), side: Side::Right, body: "nit".into() };
    let held = github(&fixtures).hold_comment(&pull(), &line).unwrap();
    assert!(held.comment.unsent);
    assert_eq!((held.thread, held.path.as_str(), held.line), (ThreadId("T_1".into()), "src/a.rs", Some(10)));
    assert_eq!(input(&fixtures, "AddReview"), json!({"pullRequestId": "PR_1"}), "no verdict: the review stays pending");
    assert_eq!(
        input(&fixtures, "Hold"),
        json!({"pullRequestReviewId": "REV_1", "path": "src/a.rs", "line": 10, "side": "RIGHT", "body": "nit", "startLine": 8, "startSide": "RIGHT"})
    );
}

#[test]
fn a_held_comment_joins_the_review_that_is_already_pending() {
    let fixtures = Fixtures::new().ok("PendingReview", pending(Some("REV_9"))).ok("Hold", held_thread("PENDING"));
    let line = NewLine { path: "a".into(), line: 1, start_line: None, side: Side::Left, body: "x".into() };
    github(&fixtures).hold_comment(&pull(), &line).unwrap();
    assert!(fixtures.sent_for("AddReview").is_empty());
    let sent = input(&fixtures, "Hold");
    assert_eq!((sent["pullRequestReviewId"].as_str(), sent["side"].as_str()), (Some("REV_9"), Some("LEFT")));
    assert!(sent.get("startLine").is_none());
}

#[test]
fn submitting_sends_the_pending_review_with_the_verdict_and_the_body() {
    for (verdict, event) in [(Verdict::Approve, "APPROVE"), (Verdict::RequestChanges, "REQUEST_CHANGES"), (Verdict::Comment, "COMMENT")] {
        let fixtures = Fixtures::new().ok("PendingReview", pending(Some("REV_9"))).ok("SubmitReview", data(json!({})));
        github(&fixtures).submit_review(&pull(), verdict, "Looks good").unwrap();
        assert_eq!(input(&fixtures, "SubmitReview"), json!({"pullRequestReviewId": "REV_9", "event": event, "body": "Looks good"}));
    }
}

#[test]
fn submitting_with_nothing_held_makes_a_review_that_carries_the_body() {
    let fixtures = Fixtures::new()
        .ok("PendingReview", pending(None))
        .ok("AddReview", data(json!({"addPullRequestReview": {"pullRequestReview": {"id": "REV_2"}}})))
        .ok("SubmitReview", data(json!({})));
    github(&fixtures).submit_review(&pull(), Verdict::Approve, "LGTM").unwrap();
    assert_eq!(input(&fixtures, "AddReview"), json!({"pullRequestId": "PR_1", "body": "LGTM"}));
    assert_eq!(input(&fixtures, "SubmitReview")["pullRequestReviewId"], "REV_2");
}

#[test]
fn a_reply_goes_to_the_thread_and_comes_back_as_the_comment() {
    let comment = json!({"id": "C_2", "body": "Done", "createdAt": "2026-09-29T15:14:16Z", "author": {"login": "ada", "__typename": "User"}});
    let fixtures = Fixtures::new().ok("Reply", data(json!({"addPullRequestReviewThreadReply": {"comment": comment}})));
    let made = github(&fixtures).reply(&ThreadId("T_1".into()), "Done").unwrap();
    assert_eq!(made.id, "C_2");
    assert_eq!(input(&fixtures, "Reply"), json!({"pullRequestReviewThreadId": "T_1", "body": "Done"}));
}

#[test]
fn a_thread_is_resolved_and_unresolved_by_two_mutations() {
    let fixtures = Fixtures::new().ok("Resolve", data(json!({}))).ok("Unresolve", data(json!({})));
    let forge = github(&fixtures);
    forge.resolve(&ThreadId("T_1".into()), true).unwrap();
    forge.resolve(&ThreadId("T_1".into()), false).unwrap();
    assert_eq!(input(&fixtures, "Resolve"), json!({"threadId": "T_1"}));
    assert_eq!(input(&fixtures, "Unresolve"), json!({"threadId": "T_1"}));
}

#[test]
fn a_remote_that_is_not_github_is_unknown_and_sends_nothing() {
    let fixtures = Fixtures::new();
    let forge = github(&fixtures);
    for url in ["https://gitlab.com/o/r.git", "not a url", "https://github.com/o", "git@github.com:o/r/extra.git"] {
        assert!(matches!(forge.repository(url), Err(ForgeError::UnknownRemote(_))), "{url}");
    }
    assert!(fixtures.sent().is_empty());
}

#[test]
fn a_signed_out_gh_and_an_offline_one_reach_the_caller_as_their_own_errors() {
    let out = Fixtures::new().fails("ForWrite", TransportError::NotSignedIn);
    assert_eq!(github(&out).merge(&pull(), &merge_request(MergeMethod::Merge)).err().unwrap(), ForgeError::NotSignedIn);
    let off = Fixtures::new().fails("ForWrite", TransportError::Offline);
    assert_eq!(github(&off).merge(&pull(), &merge_request(MergeMethod::Merge)).err().unwrap(), ForgeError::Offline);
}
