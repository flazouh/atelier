//! Changes lathe asks GitHub to make. Each is one mutation, or REST where GraphQL has none. Nothing
//! here retries a write on its own: a change that may have landed is never sent twice. The client
//! retries a rate limit, which GitHub refuses before it acts, and a read that met a server error.
use serde_json::{Value, json};

use super::{GitHub, decode, queries, read, wire::Root};
use crate::{
    Comment, ForgeError, ForgeResult, HeldComment, MergeMethod, MergeOutcome, MergeRequest, NewLine, NewPull,
    PullRef, PullUpdate, RepoRef, Reviewer, Side, ThreadId, UpdateMethod, Verdict,
};

const CREATE_PULL: &str = "mutation CreatePull($input: CreatePullRequestInput!) {
  createPullRequest(input: $input) { pullRequest { number repository { nameWithOwner } } } }";
const UPDATE_PULL: &str = "mutation UpdatePull($input: UpdatePullRequestInput!) {
  updatePullRequest(input: $input) { pullRequest { id } } }";
const READY: &str = "mutation Ready($input: MarkPullRequestReadyForReviewInput!) {
  markPullRequestReadyForReview(input: $input) { pullRequest { id } } }";
const DRAFT: &str = "mutation Draft($input: ConvertPullRequestToDraftInput!) {
  convertPullRequestToDraft(input: $input) { pullRequest { id } } }";
const MERGE: &str = "mutation Merge($input: MergePullRequestInput!) {
  mergePullRequest(input: $input) { pullRequest { id merged } } }";
const AUTO_MERGE: &str = "mutation AutoMerge($input: EnablePullRequestAutoMergeInput!) {
  enablePullRequestAutoMerge(input: $input) { pullRequest { id } } }";
const ENQUEUE: &str = "mutation Enqueue($input: EnqueuePullRequestInput!) {
  enqueuePullRequest(input: $input) { mergeQueueEntry { id } } }";
const UPDATE_BRANCH: &str = "mutation UpdateBranch($input: UpdatePullRequestBranchInput!) {
  updatePullRequestBranch(input: $input) { pullRequest { id } } }";
const DISABLE_AUTO_MERGE: &str = "mutation DisableAutoMerge($input: DisablePullRequestAutoMergeInput!) {
  disablePullRequestAutoMerge(input: $input) { pullRequest { id } } }";
const DEQUEUE: &str = "mutation Dequeue($input: DequeuePullRequestInput!) {
  dequeuePullRequest(input: $input) { mergeQueueEntry { id } } }";
const REVERT: &str = "mutation Revert($input: RevertPullRequestInput!) {
  revertPullRequest(input: $input) { revertPullRequest { number repository { nameWithOwner } } } }";
const COMMENT_FIELDS: &str = "id body createdAt author { login __typename } state";
const ADD_COMMENT: &str = "mutation AddComment($input: AddCommentInput!) {
  addComment(input: $input) { commentEdge { node { id body createdAt author { login __typename } } } } }";
const ADD_REVIEW: &str = "mutation AddReview($input: AddPullRequestReviewInput!) {
  addPullRequestReview(input: $input) { pullRequestReview { id } } }";
const SUBMIT_REVIEW: &str = "mutation SubmitReview($input: SubmitPullRequestReviewInput!) {
  submitPullRequestReview(input: $input) { pullRequestReview { id } } }";
const RESOLVE: &str = "mutation Resolve($input: ResolveReviewThreadInput!) {
  resolveReviewThread(input: $input) { thread { id } } }";
const UNRESOLVE: &str = "mutation Unresolve($input: UnresolveReviewThreadInput!) {
  unresolveReviewThread(input: $input) { thread { id } } }";

fn method_name(method: MergeMethod) -> &'static str {
    match method {
        MergeMethod::Merge => "MERGE",
        MergeMethod::Squash => "SQUASH",
        MergeMethod::Rebase => "REBASE",
    }
}

fn event_name(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Comment => "COMMENT",
        Verdict::Approve => "APPROVE",
        Verdict::RequestChanges => "REQUEST_CHANGES",
    }
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::Left => "LEFT",
        Side::Right => "RIGHT",
    }
}

/// What a write needs to know about a pull request.
struct Target {
    id: String,
    head: String,
    head_sha: String,
    same_repo: bool,
    has_queue: bool,
}

impl GitHub {
    fn mutate(&self, query: &str, input: Value) -> ForgeResult<Value> {
        self.client.graphql(query, json!({"input": input}))?.whole()
    }

    fn target(&self, reference: &PullRef) -> ForgeResult<Target> {
        let data = self.client.graphql(queries::FOR_WRITE, Self::vars(reference))?.whole()?;
        let pull = &data["repository"]["pullRequest"];
        let text = |key: &str| pull[key].as_str().map(str::to_string);
        match (text("id"), text("headRefName"), text("headRefOid")) {
            (Some(id), Some(head), Some(head_sha)) => Ok(Target {
                id,
                head,
                head_sha,
                same_repo: pull["isCrossRepository"] == Value::Bool(false),
                has_queue: !data["repository"]["mergeQueue"].is_null(),
            }),
            _ => Err(ForgeError::NotFound(format!("{}#{}", reference.repo.slug(), reference.number))),
        }
    }

    pub(super) fn write_create_pull(&self, repo: &RepoRef, new: &NewPull) -> ForgeResult<PullRef> {
        let vars = json!({"owner": repo.owner, "name": repo.name});
        let root: Root<super::wire::Repo> = decode(self.client.graphql(queries::REPOSITORY, vars)?.whole()?)?;
        let repository_id = root.repository.ok_or_else(|| ForgeError::NotFound(repo.slug()))?.id;
        let input = json!({
            "repositoryId": repository_id, "baseRefName": new.base, "headRefName": new.head,
            "title": new.title, "body": new.body, "draft": new.draft,
        });
        let data = self.mutate(CREATE_PULL, input)?;
        let made = &data["createPullRequest"]["pullRequest"];
        let number = made["number"].as_u64();
        let repo = made["repository"]["nameWithOwner"].as_str().map(read::repo_ref);
        match (number, repo) {
            (Some(number), Some(repo)) => Ok(PullRef { repo: repo?, number }),
            _ => Err(ForgeError::Unexpected("the new pull request has no number".into())),
        }
    }

    pub(super) fn write_update_pull(&self, reference: &PullRef, update: &PullUpdate) -> ForgeResult<()> {
        let id = self.target(reference)?.id;
        let mut fields = serde_json::Map::new();
        fields.insert("pullRequestId".into(), json!(id));
        if let Some(title) = &update.title {
            fields.insert("title".into(), json!(title));
        }
        if let Some(body) = &update.body {
            fields.insert("body".into(), json!(body));
        }
        if let Some(base) = &update.base {
            fields.insert("baseRefName".into(), json!(base));
        }
        if let Some(closed) = update.closed {
            fields.insert("state".into(), json!(if closed { "CLOSED" } else { "OPEN" }));
        }
        if fields.len() > 1 {
            self.mutate(UPDATE_PULL, Value::Object(fields))?;
        }
        match update.ready {
            Some(true) => self.mutate(READY, json!({"pullRequestId": id})).map(drop),
            Some(false) => self.mutate(DRAFT, json!({"pullRequestId": id})).map(drop),
            None => Ok(()),
        }
    }

    pub(super) fn write_merge(&self, reference: &PullRef, request: &MergeRequest) -> ForgeResult<MergeOutcome> {
        let target = self.target(reference)?;
        let mut input = json!({"pullRequestId": target.id, "mergeMethod": method_name(request.method)});
        if let Some(title) = &request.title {
            input["commitHeadline"] = json!(title);
        }
        if let Some(message) = &request.message {
            input["commitBody"] = json!(message);
        }
        input["expectedHeadOid"] = json!(request.expected_head.as_deref().unwrap_or(&target.head_sha));
        // A repository with a merge queue lands nothing directly: the pull request joins the queue.
        if target.has_queue {
            self.mutate(ENQUEUE, json!({"pullRequestId": target.id, "expectedHeadOid": input["expectedHeadOid"]}))?;
            return Ok(MergeOutcome::Queued);
        }
        if request.when_ready {
            self.mutate(AUTO_MERGE, input)?;
            return Ok(MergeOutcome::WillMergeWhenReady);
        }
        self.mutate(MERGE, input)?;
        // The branch goes only after the merge landed, and never when it lives in a fork.
        if request.delete_branch && target.same_repo {
            let path = format!("repos/{}/git/refs/heads/{}", reference.repo.slug(), target.head);
            self.client.rest("DELETE", &path, None)?;
        }
        Ok(MergeOutcome::Merged)
    }

    pub(super) fn write_update_branch(&self, reference: &PullRef, method: UpdateMethod, expected_head: &str) -> ForgeResult<()> {
        let id = self.target(reference)?.id;
        let way = match method {
            UpdateMethod::Merge => "MERGE",
            UpdateMethod::Rebase => "REBASE",
        };
        self.mutate(UPDATE_BRANCH, json!({"pullRequestId": id, "expectedHeadOid": expected_head, "updateMethod": way})).map(drop)
    }

    pub(super) fn write_cancel_auto_merge(&self, reference: &PullRef) -> ForgeResult<()> {
        let id = self.target(reference)?.id;
        self.mutate(DISABLE_AUTO_MERGE, json!({"pullRequestId": id})).map(drop)
    }

    pub(super) fn write_dequeue(&self, reference: &PullRef) -> ForgeResult<()> {
        let id = self.target(reference)?.id;
        self.mutate(DEQUEUE, json!({"id": id})).map(drop)
    }

    pub(super) fn write_delete_branch(&self, reference: &PullRef) -> ForgeResult<()> {
        let target = self.target(reference)?;
        if !target.same_repo {
            return Err(ForgeError::Rejected("The branch lives in a fork; only its owner can delete it.".into()));
        }
        let path = format!("repos/{}/git/refs/heads/{}", reference.repo.slug(), target.head);
        self.client.rest("DELETE", &path, None).map(drop)
    }

    pub(super) fn write_revert(&self, reference: &PullRef) -> ForgeResult<PullRef> {
        let id = self.target(reference)?.id;
        let data = self.mutate(REVERT, json!({"pullRequestId": id}))?;
        let made = &data["revertPullRequest"]["revertPullRequest"];
        let number = made["number"].as_u64();
        let repo = made["repository"]["nameWithOwner"].as_str().map(read::repo_ref);
        match (number, repo) {
            (Some(number), Some(repo)) => Ok(PullRef { repo: repo?, number }),
            _ => Err(ForgeError::Unexpected("the revert has no pull request number".into())),
        }
    }

    pub(super) fn write_request_review(&self, reference: &PullRef, reviewers: &[Reviewer]) -> ForgeResult<()> {
        let people: Vec<&str> = reviewers.iter().filter_map(|r| if let Reviewer::Person(p) = r { Some(p.as_str()) } else { None }).collect();
        let teams: Vec<&str> = reviewers.iter().filter_map(|r| if let Reviewer::Team(t) = r { Some(t.as_str()) } else { None }).collect();
        let path = format!("repos/{}/pulls/{}/requested_reviewers", reference.repo.slug(), reference.number);
        self.client.rest("POST", &path, Some(json!({"reviewers": people, "team_reviewers": teams}))).map(drop)
    }

    pub(super) fn write_comment(&self, reference: &PullRef, body: &str) -> ForgeResult<Comment> {
        let id = self.target(reference)?.id;
        let data = self.mutate(ADD_COMMENT, json!({"subjectId": id, "body": body}))?;
        let node = decode(data["addComment"]["commentEdge"]["node"].clone())?;
        Ok(read::comment(&node))
    }

    /// The reader's pending review on this pull request, made when there is none.
    fn pending_review(&self, reference: &PullRef, body: Option<&str>) -> ForgeResult<String> {
        let data = self.client.graphql(queries::PENDING_REVIEW, Self::vars(reference))?.whole()?;
        let pull = &data["repository"]["pullRequest"];
        if let Some(id) = pull["reviews"]["nodes"][0]["id"].as_str() {
            return Ok(id.to_string());
        }
        let pull_id = pull["id"]
            .as_str()
            .ok_or_else(|| ForgeError::NotFound(format!("{}#{}", reference.repo.slug(), reference.number)))?;
        let mut input = json!({"pullRequestId": pull_id});
        if let Some(body) = body {
            input["body"] = json!(body);
        }
        let made = self.mutate(ADD_REVIEW, input)?;
        made["addPullRequestReview"]["pullRequestReview"]["id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| ForgeError::Unexpected("the new review has no id".into()))
    }

    pub(super) fn write_hold_comment(&self, reference: &PullRef, line: &NewLine) -> ForgeResult<HeldComment> {
        let review = self.pending_review(reference, None)?;
        let mut input = json!({
            "pullRequestReviewId": review, "path": line.path, "line": line.line,
            "side": side_name(line.side), "body": line.body,
        });
        if let Some(start) = line.start_line {
            input["startLine"] = json!(start);
            input["startSide"] = json!(side_name(line.side));
        }
        let query = format!(
            "mutation Hold($input: AddPullRequestReviewThreadInput!) {{
              addPullRequestReviewThread(input: $input) {{ thread {{ id comments(first: 1) {{ nodes {{ {COMMENT_FIELDS} }} }} }} }} }}"
        );
        let data = self.mutate(&query, input)?;
        let thread = &data["addPullRequestReviewThread"]["thread"];
        let id = thread["id"].as_str().ok_or_else(|| ForgeError::Unexpected("the new thread has no id".into()))?;
        let node = decode(thread["comments"]["nodes"][0].clone())?;
        Ok(HeldComment { thread: ThreadId(id.to_string()), comment: read::comment(&node), path: line.path.clone(), line: Some(line.line) })
    }

    pub(super) fn write_submit_review(&self, reference: &PullRef, verdict: Verdict, body: &str) -> ForgeResult<()> {
        let review = self.pending_review(reference, Some(body))?;
        self.mutate(SUBMIT_REVIEW, json!({"pullRequestReviewId": review, "event": event_name(verdict), "body": body})).map(drop)
    }

    pub(super) fn write_reply(&self, thread: &ThreadId, body: &str) -> ForgeResult<Comment> {
        let query = format!(
            "mutation Reply($input: AddPullRequestReviewThreadReplyInput!) {{
              addPullRequestReviewThreadReply(input: $input) {{ comment {{ {COMMENT_FIELDS} }} }} }}"
        );
        let data = self.mutate(&query, json!({"pullRequestReviewThreadId": thread.0, "body": body}))?;
        Ok(read::comment(&decode(data["addPullRequestReviewThreadReply"]["comment"].clone())?))
    }

    pub(super) fn write_resolve(&self, thread: &ThreadId, resolved: bool) -> ForgeResult<()> {
        let query = if resolved { RESOLVE } else { UNRESOLVE };
        self.mutate(query, json!({"threadId": thread.0})).map(drop)
    }
}
