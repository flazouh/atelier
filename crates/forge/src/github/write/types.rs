pub(super) const CREATE_PULL: &str = "mutation CreatePull($input: CreatePullRequestInput!) {
  createPullRequest(input: $input) { pullRequest { number repository { nameWithOwner } } } }";

pub(super) const UPDATE_PULL: &str = "mutation UpdatePull($input: UpdatePullRequestInput!) {
  updatePullRequest(input: $input) { pullRequest { id } } }";

pub(super) const READY: &str = "mutation Ready($input: MarkPullRequestReadyForReviewInput!) {
  markPullRequestReadyForReview(input: $input) { pullRequest { id } } }";

pub(super) const DRAFT: &str = "mutation Draft($input: ConvertPullRequestToDraftInput!) {
  convertPullRequestToDraft(input: $input) { pullRequest { id } } }";

pub(super) const MERGE: &str = "mutation Merge($input: MergePullRequestInput!) {
  mergePullRequest(input: $input) { pullRequest { id merged } } }";

pub(super) const AUTO_MERGE: &str = "mutation AutoMerge($input: EnablePullRequestAutoMergeInput!) {
  enablePullRequestAutoMerge(input: $input) { pullRequest { id } } }";

pub(super) const ENQUEUE: &str = "mutation Enqueue($input: EnqueuePullRequestInput!) {
  enqueuePullRequest(input: $input) { mergeQueueEntry { id } } }";

pub(super) const UPDATE_BRANCH: &str = "mutation UpdateBranch($input: UpdatePullRequestBranchInput!) {
  updatePullRequestBranch(input: $input) { pullRequest { id } } }";

pub(super) const DISABLE_AUTO_MERGE: &str = "mutation DisableAutoMerge($input: DisablePullRequestAutoMergeInput!) {
  disablePullRequestAutoMerge(input: $input) { pullRequest { id } } }";

pub(super) const DEQUEUE: &str = "mutation Dequeue($input: DequeuePullRequestInput!) {
  dequeuePullRequest(input: $input) { mergeQueueEntry { id } } }";

pub(super) const REVERT: &str = "mutation Revert($input: RevertPullRequestInput!) {
  revertPullRequest(input: $input) { revertPullRequest { number repository { nameWithOwner } } } }";

pub(super) const COMMENT_FIELDS: &str = "id body createdAt author { login __typename } state";

pub(super) const ADD_COMMENT: &str = "mutation AddComment($input: AddCommentInput!) {
  addComment(input: $input) { commentEdge { node { id body createdAt author { login __typename } } } } }";

pub(super) const ADD_REVIEW: &str = "mutation AddReview($input: AddPullRequestReviewInput!) {
  addPullRequestReview(input: $input) { pullRequestReview { id } } }";

pub(super) const SUBMIT_REVIEW: &str = "mutation SubmitReview($input: SubmitPullRequestReviewInput!) {
  submitPullRequestReview(input: $input) { pullRequestReview { id } } }";

pub(super) const RESOLVE: &str = "mutation Resolve($input: ResolveReviewThreadInput!) {
  resolveReviewThread(input: $input) { thread { id } } }";

pub(super) const UNRESOLVE: &str = "mutation Unresolve($input: UnresolveReviewThreadInput!) {
  unresolveReviewThread(input: $input) { thread { id } } }";
