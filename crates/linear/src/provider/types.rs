//! The constants of the provider: where Linear is, and the GraphQL documents it is asked.

/// Linear's one GraphQL address.
pub const ENDPOINT: &str = "https://api.linear.app/graphql";

/// How often `subscribe` asks what changed.
pub const POLL_EVERY: std::time::Duration = std::time::Duration::from_secs(30);

/// How often the polling thread looks at whether its subscription was dropped.
pub const STOP_CHECK: std::time::Duration = std::time::Duration::from_millis(100);

/// The most Linear lets one page hold.
pub const PAGE_MAX: u32 = 250;

/// How many a list gives when the query names no limit.
pub const PAGE_DEFAULT: u32 = 50;

/// The id of a label, a project or a comment is a Linear uuid. In a reference it follows one of these words, so a
/// reference says what it points at and cannot be mistaken for a task key.
pub const LABEL: &str = "label";
pub const PROJECT: &str = "project";
pub const COMMENT: &str = "comment";
pub const HISTORY: &str = "history";
pub const CREATED: &str = "created";

/// The fields of an issue, read by every call that gives a task. Added after each document that names it.
pub const ISSUE_FIELDS: &str = "fragment IssueFields on Issue { id identifier title description priority estimate \
dueDate createdAt updatedAt url state { id name type } assignee { id name } creator { id name } labels { nodes { id name color } } \
project { id name slugId } parent { id identifier } team { id key } }";

pub const VIEWER: &str = "query Viewer { viewer { id name organization { urlKey } } }";

pub const TEAMS: &str = "query Teams { teams(first: 100) { nodes { id key name } } }";

pub const LIST: &str = "query List($first: Int!, $after: String, $filter: IssueFilter, $orderBy: PaginationOrderBy) { \
issues(first: $first, after: $after, filter: $filter, orderBy: $orderBy) { nodes { ...IssueFields } \
pageInfo { hasNextPage endCursor } } }";

pub const GET: &str = "query Get($id: String!) { issue(id: $id) { ...IssueFields } }";

pub const CREATE: &str = "mutation Create($input: IssueCreateInput!) { issueCreate(input: $input) { success \
issue { ...IssueFields } } }";

pub const UPDATE: &str = "mutation Update($id: String!, $input: IssueUpdateInput!) { issueUpdate(id: $id, input: $input) { \
success issue { ...IssueFields } } }";

pub const COMMENT_CREATE: &str = "mutation Comment($input: CommentCreateInput!) { commentCreate(input: $input) { success \
comment { id body createdAt updatedAt user { id name } } } }";

pub const COMMENTS: &str = "query Comments($id: String!, $first: Int!, $after: String) { issue(id: $id) { \
comments(first: $first, after: $after) { nodes { id body createdAt updatedAt user { id name } botActor { name } } \
pageInfo { hasNextPage endCursor } } } }";

pub const HISTORY_OF: &str = "query History($id: String!, $first: Int!, $after: String) { issue(id: $id) { id createdAt \
creator { id name } history(first: $first, after: $after) { nodes { id createdAt actor { id name } botActor { name } \
fromState { id name type } toState { id name type } fromAssignee { id name } toAssignee { id name } fromTitle toTitle \
updatedDescription fromPriority toPriority addedLabels { id name } removedLabels { id name } \
fromProject { id name } toProject { id name } } pageInfo { hasNextPage endCursor } } } }";

pub const LABELS: &str = "query Labels($first: Int!, $after: String) { issueLabels(first: $first, after: $after) { \
nodes { id name color } pageInfo { hasNextPage endCursor } } }";

pub const PROJECTS: &str = "query Projects($first: Int!, $after: String) { projects(first: $first, after: $after) { \
nodes { id name slugId } pageInfo { hasNextPage endCursor } } }";

pub const STATES: &str = "query States($first: Int!, $after: String, $filter: WorkflowStateFilter) { \
workflowStates(first: $first, after: $after, filter: $filter) { nodes { id name type position team { key } } \
pageInfo { hasNextPage endCursor } } }";

/// The newest change in the workspace, for the starting point of a subscription.
pub const LATEST: &str = "query Latest($filter: IssueFilter) { issues(first: 1, orderBy: updatedAt, filter: $filter) { \
nodes { updatedAt } } }";
