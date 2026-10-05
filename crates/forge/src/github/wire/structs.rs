use serde::Deserialize;

use super::helpers::one;

#[derive(Deserialize)]
pub(in super::super) struct Page<T> {
    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
    pub nodes: Vec<Option<T>>,
}

#[derive(Deserialize)]
pub(in super::super) struct PageInfo {
    #[serde(rename = "hasNextPage")]
    pub has_next_page: bool,
    #[serde(rename = "endCursor")]
    pub end_cursor: Option<String>,
}

impl PageInfo {
    /// The cursor to ask for the next page with, or `None` on the last.
    pub fn next(&self) -> Option<String> {
        self.has_next_page.then(|| self.end_cursor.clone()).flatten()
    }
}

#[derive(Deserialize)]
pub(in super::super) struct Root<T> {
    pub repository: Option<T>,
}

#[derive(Deserialize)]
pub(in super::super) struct Login {
    pub login: String,
    #[serde(rename = "__typename", default)]
    pub typename: String,
}

#[derive(Deserialize)]
pub(in super::super) struct Count {
    #[serde(rename = "totalCount")]
    pub total: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Repo {
    pub id: String,
    pub name_with_owner: String,
    pub url: Option<String>,
    pub default_branch_ref: Option<Named>,
    #[serde(default)]
    pub merge_commit_allowed: bool,
    #[serde(default)]
    pub squash_merge_allowed: bool,
    #[serde(default)]
    pub rebase_merge_allowed: bool,
    #[serde(default)]
    pub auto_merge_allowed: bool,
    #[serde(default)]
    pub delete_branch_on_merge: bool,
    pub viewer_default_merge_method: Option<String>,
    pub viewer_permission: Option<String>,
    pub merge_queue: Option<serde_json::Value>,
    pub pull_request: Option<PullNode>,
}

#[derive(Deserialize)]
pub(in super::super) struct Named {
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct PullNode {
    pub id: String,
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: String,
    pub state: String,
    #[serde(default)]
    pub is_draft: bool,
    #[serde(default)]
    pub merged: bool,
    pub url: String,
    pub created_at: String,
    pub updated_at: String,
    pub author: Option<Login>,
    pub base_ref_name: String,
    /// The base branch's commit the pull request stands on. Older recordings lack it.
    #[serde(default)]
    pub base_ref_oid: String,
    pub head_ref_name: String,
    pub head_ref_oid: String,
    pub mergeable: Option<String>,
    pub merge_state_status: Option<String>,
    pub review_decision: Option<String>,
    #[serde(default)]
    pub additions: u32,
    #[serde(default)]
    pub deletions: u32,
    #[serde(default)]
    pub changed_files: u32,
    pub comments: Count,
    #[serde(default)]
    pub viewer_can_update: bool,
    #[serde(default)]
    pub viewer_can_merge_as_admin: bool,
    pub viewer_latest_review: Option<LatestReview>,
    #[serde(default)]
    pub is_in_merge_queue: bool,
    pub merge_queue_entry: Option<QueueEntry>,
    pub auto_merge_request: Option<serde_json::Value>,
    pub review_requests: Option<Nodes<RequestNode>>,
    pub latest_reviews: Option<Nodes<ReviewNode>>,
    pub commits: Option<Nodes<CommitNode>>,
}

#[derive(Deserialize)]
pub(in super::super) struct Nodes<T> {
    #[serde(default = "Vec::new")]
    pub nodes: Vec<Option<T>>,
}

#[derive(Deserialize)]
pub(in super::super) struct QueueEntry {
    pub position: Option<u32>,
}

#[derive(Deserialize)]
pub(in super::super) struct LatestReview {
    pub commit: Option<Oid>,
}

#[derive(Deserialize)]
pub(in super::super) struct Oid {
    pub oid: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct RequestNode {
    pub requested_reviewer: Option<Reviewer>,
}

#[derive(Deserialize)]
pub(in super::super) struct Reviewer {
    #[serde(rename = "__typename")]
    pub typename: String,
    pub login: Option<String>,
    pub slug: Option<String>,
}

#[derive(Deserialize)]
pub(in super::super) struct ReviewNode {
    pub author: Option<Login>,
    pub state: String,
}

#[derive(Deserialize)]
pub(in super::super) struct CommitNode {
    pub commit: CommitInfo,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct CommitInfo {
    pub status_check_rollup: Option<Rollup>,
}

#[derive(Deserialize)]
pub(in super::super) struct Rollup {
    pub contexts: Contexts,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Contexts {
    #[serde(default)]
    pub check_run_counts_by_state: Vec<StateCount>,
    #[serde(default)]
    pub status_context_counts_by_state: Vec<StateCount>,
}

#[derive(Deserialize)]
pub(in super::super) struct StateCount {
    pub state: String,
    pub count: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct FileNode {
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
    pub change_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ThreadNode {
    pub id: String,
    pub is_resolved: bool,
    pub is_outdated: bool,
    pub path: String,
    pub line: Option<u32>,
    pub start_line: Option<u32>,
    pub original_line: Option<u32>,
    pub subject_type: Option<String>,
    pub diff_side: Option<String>,
    #[serde(default)]
    pub viewer_can_resolve: bool,
    #[serde(default)]
    pub viewer_can_reply: bool,
    pub comments: Page<CommentNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct CommentNode {
    pub id: String,
    pub body: String,
    pub created_at: String,
    pub author: Option<Login>,
    pub state: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ContextNode {
    #[serde(rename = "__typename")]
    pub typename: String,
    // A check run.
    pub database_id: Option<u64>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub conclusion: Option<String>,
    pub details_url: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub check_suite: Option<Suite>,
    // A commit status.
    pub context: Option<String>,
    pub state: Option<String>,
    pub target_url: Option<String>,
    pub created_at: Option<String>,
    #[serde(default)]
    pub is_required: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Suite {
    pub conclusion: Option<String>,
    pub workflow_run: Option<WorkflowRun>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct WorkflowRun {
    pub database_id: u64,
    #[serde(default)]
    pub run_number: u64,
    #[serde(default)]
    pub event: String,
    pub workflow: Option<Named>,
}

/// One row of a search: a pull request, or `{}` for a hit that is not one.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct SearchHit {
    pub number: Option<u64>,
    pub title: Option<String>,
    pub state: Option<String>,
    #[serde(default)]
    pub is_draft: bool,
    #[serde(default)]
    pub merged: bool,
    pub url: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub repository: Option<RepoName>,
    pub author: Option<Login>,
    pub review_decision: Option<String>,
    #[serde(default)]
    pub additions: u32,
    #[serde(default)]
    pub deletions: u32,
    pub comments: Option<Count>,
    pub commits: Option<Nodes<CommitNode>>,
    #[serde(default)]
    pub head_ref_name: String,
    #[serde(default)]
    pub base_ref_name: String,
    pub mergeable: Option<String>,
    #[serde(default)]
    pub is_in_merge_queue: bool,
    pub merge_queue_entry: Option<QueueEntry>,
    pub auto_merge_request: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct RepoName {
    pub name_with_owner: String,
}

/// A REST job: `GET repos/o/r/actions/jobs/{id}`.
#[derive(Deserialize)]
pub(in super::super) struct RestJob {
    pub id: u64,
    pub run_id: u64,
    #[serde(default = "one")]
    pub run_attempt: u32,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    #[serde(default)]
    pub steps: Vec<RestStep>,
}

#[derive(Deserialize)]
pub(in super::super) struct RestStep {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub number: u32,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}
