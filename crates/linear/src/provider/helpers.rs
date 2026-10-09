//! The free functions of the provider: Linear's JSON into the neutral entities, and the neutral calls into Linear's
//! variables. None of them touches the network except `collect`, `latest_update` and `changes_since`, which page
//! through a list.

use atelier_capabilities::{
    Actor, ActorKind, CapError, CapResult, Ref,
    tasks::{
        Activity, ActivityKind, Category, Change, Comment, Event, EventKind, Label, NewTask, Page,
        Patch, Priority, Project, Query, Sort, Status, Task,
    },
};
use chrono::{DateTime, NaiveDate};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use super::{
    structs::{
        BotNode, CommentNode, HistoryNode, IssueNode, LabelNode, ProjectNode, StateNode, UserNode,
    },
    types::{self, COMMENT, CREATED, HISTORY, LABEL, PAGE_DEFAULT, PAGE_MAX, PROJECT},
};
use crate::client::Client;

/// Reads a node into its typed form. A node that does not fit is Linear changing under us, so it is a provider error.
pub fn read<T: DeserializeOwned>(value: &Value, what: &str) -> CapResult<T> {
    serde_json::from_value(value.clone()).map_err(|e| CapError::Provider {
        code: "unexpected_shape".into(),
        message: format!("a {what} from Linear did not have the expected fields: {e}"),
    })
}

pub fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_string()
}

/// Adds the issue fragment to a document that spreads it.
pub fn with_issue_fields(document: &str) -> String {
    format!("{document} {}", types::ISSUE_FIELDS)
}

/// The `data` of a create or an update, which holds the new thing next to `success`.
pub fn mutated(result: &Value, field: &str) -> CapResult<Value> {
    if result["success"].as_bool() == Some(true) && !result[field].is_null() {
        Ok(result[field].clone())
    } else {
        Err(CapError::Provider {
            code: "not_done".into(),
            message: "Linear did not do it".into(),
        })
    }
}

// Categories, priorities, time.

/// The spec's mapping, section 4.2. A started state whose name says "review" is `in_review`. Linear's `triage`
/// is a waiting place before the backlog.
pub fn category_of(kind: &str, name: &str) -> Option<Category> {
    Some(match kind {
        "triage" | "backlog" => Category::Backlog,
        "unstarted" => Category::Todo,
        "started" if name.to_lowercase().contains("review") => Category::InReview,
        "started" => Category::InProgress,
        "completed" => Category::Done,
        // Linear also has a state type of its own for a duplicate.
        "canceled" | "duplicate" => Category::Canceled,
        _ => return None,
    })
}

pub fn status_from(node: &Value) -> CapResult<Status> {
    status_of(&read::<StateNode>(node, "state")?)
}

pub fn status_of(state: &StateNode) -> CapResult<Status> {
    let category = category_of(&state.kind, &state.name).ok_or_else(|| CapError::Provider {
        code: "unknown_state_type".into(),
        message: format!(
            "Linear has a state type `{}` this provider does not know",
            state.kind
        ),
    })?;
    Ok(Status {
        id: state.id.clone(),
        name: state.name.clone(),
        category,
    })
}

/// Milliseconds since the epoch of Linear's ISO 8601 time.
pub fn millis(iso: &str) -> CapResult<i64> {
    DateTime::parse_from_rfc3339(iso)
        .map(|t| t.timestamp_millis())
        .map_err(|_| CapError::Provider {
            code: "unexpected_shape".into(),
            message: format!("`{iso}` is not a time"),
        })
}

/// A due date is a day with no time: it is midnight UTC.
fn due_millis(day: &str) -> CapResult<i64> {
    NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|t| t.and_utc().timestamp_millis())
        .ok_or_else(|| CapError::Provider {
            code: "unexpected_shape".into(),
            message: format!("`{day}` is not a day"),
        })
}

// References.

pub fn task_ref(account: &str, key: &str) -> CapResult<Ref> {
    Ref::new("tasks", "linear", account, key).map_err(|_| CapError::invalid("ref"))
}

/// The reference of a label, a project, a comment or a history entry: its kind word, a colon, its Linear id.
pub fn scoped_ref(account: &str, word: &str, id: &str) -> CapResult<Ref> {
    task_ref(account, &format!("{word}:{id}"))
}

/// The Linear key of a task reference: `ENG-350`. A reference of another provider, another workspace, or another
/// kind of thing is not a task here.
pub fn task_key(task: &Ref, account: &str) -> CapResult<String> {
    let ours = task.capability == "tasks" && task.provider == "linear" && task.account == account;
    if ours && !task.id.contains(':') {
        Ok(task.id.clone())
    } else {
        Err(CapError::not_found(task.to_string()))
    }
}

/// The Linear id inside a scoped reference, for example the uuid of a label.
pub fn scoped_id(reference: &Ref, word: &str, account: &str, field: &str) -> CapResult<String> {
    let prefix = format!("{word}:");
    match reference.id.strip_prefix(&prefix) {
        Some(id) if reference.provider == "linear" && reference.account == account => {
            Ok(id.to_string())
        }
        _ => Err(CapError::invalid(field)),
    }
}

// Entities.

pub fn person(user: &UserNode) -> Actor {
    Actor::person(&user.id, &user.name)
}

/// Who did something: a person, an app such as GitHub, or nobody Linear can name.
fn who(user: Option<&UserNode>, bot: Option<&BotNode>) -> Actor {
    match (user, bot) {
        (Some(user), _) => person(user),
        (None, Some(bot)) => Actor {
            kind: ActorKind::Agent,
            id: format!("app:{}", bot.name),
            name: bot.name.clone(),
            on_behalf_of: None,
        },
        (None, None) => Actor::person("linear", "Linear"),
    }
}

pub fn task_from(account: &str, raw: &Value) -> CapResult<Task> {
    let issue: IssueNode = read(raw, "issue")?;
    let labels = issue
        .labels
        .iter()
        .flat_map(|l| &l.nodes)
        .map(|l| scoped_ref(account, LABEL, &l.id))
        .collect::<CapResult<Vec<_>>>()?;
    Ok(Task {
        reference: task_ref(account, &issue.identifier)?,
        key: issue.identifier.clone(),
        title: issue.title,
        description: issue.description.unwrap_or_default(),
        status: status_of(&issue.state)?,
        priority: Priority::from_number(issue.priority).ok_or_else(|| CapError::Provider {
            code: "unexpected_shape".into(),
            message: format!("`{}` is not a Linear priority", issue.priority),
        })?,
        assignees: issue.assignee.iter().map(person).collect(),
        labels,
        project: issue
            .project
            .map(|p| scoped_ref(account, PROJECT, &p.id))
            .transpose()?,
        parent: issue
            .parent
            .map(|p| task_ref(account, &p.identifier))
            .transpose()?,
        links: vec![],
        created_at: millis(&issue.created_at)?,
        updated_at: millis(&issue.updated_at)?,
        // `updatedAt` moves with every change, and nothing else of Linear's does, so it is the version.
        version: issue.updated_at,
        due_at: issue.due_date.as_deref().map(due_millis).transpose()?,
        estimate: issue.estimate,
        raw: Some(raw.clone()),
    })
}

pub fn task_page(account: &str, connection: &Value) -> CapResult<Page<Task>> {
    let items = connection["nodes"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|n| task_from(account, n))
        .collect::<CapResult<Vec<_>>>()?;
    Ok(Page {
        items,
        next_cursor: next_cursor(connection),
    })
}

/// Linear's cursor for the next page, when there is one.
fn next_cursor(connection: &Value) -> Option<String> {
    let page = &connection["pageInfo"];
    if page["hasNextPage"].as_bool() == Some(true) {
        page["endCursor"].as_str().map(str::to_string)
    } else {
        None
    }
}

pub fn label_from(account: &str, raw: &Value) -> CapResult<Label> {
    let label: LabelNode = read(raw, "label")?;
    Ok(Label {
        reference: scoped_ref(account, LABEL, &label.id)?,
        name: label.name,
        color: label.color,
        raw: Some(raw.clone()),
    })
}

pub fn project_from(account: &str, raw: &Value) -> CapResult<Project> {
    let project: ProjectNode = read(raw, "project")?;
    Ok(Project {
        reference: scoped_ref(account, PROJECT, &project.id)?,
        key: project.slug_id,
        name: project.name,
        raw: Some(raw.clone()),
    })
}

pub fn comment_from(account: &str, task: &Ref, raw: &Value) -> CapResult<Comment> {
    let comment: CommentNode = read(raw, "comment")?;
    Ok(Comment {
        reference: scoped_ref(account, COMMENT, &comment.id)?,
        task: task.clone(),
        author: who(comment.user.as_ref(), comment.bot_actor.as_ref()),
        body: comment.body,
        created_at: millis(&comment.created_at)?,
        updated_at: comment.updated_at.as_deref().map(millis).transpose()?,
        raw: Some(raw.clone()),
    })
}

/// Linear writes a comment as the owner of the key and has no place for who asked. An agent's comment says so in
/// its last line, so a reader sees it was not the person.
pub fn comment_body(body: &str, by: &Actor) -> String {
    match by.kind {
        ActorKind::Person => body.to_string(),
        ActorKind::Agent => format!("{body}\n\n_Written by the agent {}._", by.name),
    }
}

/// The history entry as the one kind of activity it most plainly is. An entry that changes a state and a label at
/// once is a status change; the whole entry stays in `detail`.
fn history_activity(account: &str, task: &Ref, raw: &Value) -> CapResult<Activity> {
    let entry: HistoryNode = read(raw, "history entry")?;
    let kind = if entry.to_state.is_some() {
        ActivityKind::StatusChanged
    } else if entry.to_assignee.is_some() || entry.from_assignee.is_some() {
        ActivityKind::Assigned
    } else {
        ActivityKind::Edited
    };
    Ok(Activity {
        reference: scoped_ref(account, HISTORY, &entry.id)?,
        task: task.clone(),
        at: millis(&entry.created_at)?,
        by: who(entry.actor.as_ref(), entry.bot_actor.as_ref()),
        kind,
        detail: Some(raw.clone()),
    })
}

/// One page of the activity of a task, oldest first: its creation, its history and its comments in one list.
/// Linear pages history and comments apart, so the whole list is built and the cursor is a position in it.
pub fn activity_page(
    account: &str,
    task: &Ref,
    issue: &Value,
    history: &[Value],
    comments: &[Value],
    cursor: Option<&str>,
) -> CapResult<Page<Activity>> {
    let created: IssueNode = read(issue, "issue")?;
    let mut all = vec![Activity {
        reference: scoped_ref(account, CREATED, &text(&issue["id"]))?,
        task: task.clone(),
        at: millis(&created.created_at)?,
        by: created
            .creator
            .as_ref()
            .map_or_else(|| who(None, None), person),
        kind: ActivityKind::Created,
        detail: None,
    }];
    for entry in history {
        all.push(history_activity(account, task, entry)?);
    }
    for raw in comments {
        let comment = comment_from(account, task, raw)?;
        all.push(Activity {
            reference: comment.reference,
            task: task.clone(),
            at: comment.created_at,
            by: comment.author,
            kind: ActivityKind::Commented,
            detail: Some(json!({ "body": comment.body })),
        });
    }
    all.sort_by_key(|a| (a.at, a.reference.to_string()));
    let start = match cursor {
        None => 0,
        Some(text) => text
            .parse::<usize>()
            .map_err(|_| CapError::invalid("cursor"))?,
    };
    let end = (start + PAGE_DEFAULT as usize).min(all.len());
    let items = all.get(start..end).unwrap_or_default().to_vec();
    Ok(Page {
        items,
        next_cursor: (end < all.len()).then(|| end.to_string()),
    })
}

// Queries into variables.

/// Linear can order by when something was last changed or made, and by nothing else.
pub fn order_by(query: &Query) -> CapResult<&'static str> {
    match query.sort {
        Sort::Updated => Ok("updatedAt"),
        Sort::Created => Ok("createdAt"),
        Sort::Priority => Err(CapError::unsupported("sort by priority")),
    }
}

/// The filter of a list. Statuses become Linear state types, so `in_progress` and `in_review` split one type by
/// the word "review" in the state's name, the same rule that maps a state to a category.
pub fn issue_filter(query: &Query, team: Option<&str>, account: &str) -> CapResult<Value> {
    if query.linked_to.is_some() {
        return Err(CapError::unsupported("list tasks by what they link to"));
    }
    let mut all = vec![];
    if let Some(team) = team {
        all.push(json!({ "team": { "key": { "eq": team } } }));
    }
    if !query.status.is_empty() {
        let any: Vec<Value> = query.status.iter().map(|c| state_filter(*c)).collect();
        all.push(json!({ "or": any }));
    }
    if let Some(assignee) = &query.assignee {
        all.push(json!({ "assignee": { "id": { "eq": assignee } } }));
    }
    if let Some(project) = &query.project {
        let id = scoped_id(project, PROJECT, account, "project")?;
        all.push(json!({ "project": { "id": { "eq": id } } }));
    }
    for label in &query.labels {
        let id = scoped_id(label, LABEL, account, "labels")?;
        all.push(json!({ "labels": { "some": { "id": { "eq": id } } } }));
    }
    if let Some(text) = query.text.as_deref().filter(|t| !t.trim().is_empty()) {
        all.push(json!({ "searchableContent": { "contains": text } }));
    }
    Ok(match all.len() {
        0 => Value::Null,
        1 => all.remove(0),
        _ => json!({ "and": all }),
    })
}

fn state_filter(category: Category) -> Value {
    let state_type = |kind: &str| json!({ "state": { "type": { "eq": kind } } });
    let started = |review: &str| json!({ "state": { "type": { "eq": "started" }, "name": { review: "review" } } });
    match category {
        Category::Backlog => json!({ "state": { "type": { "in": ["backlog", "triage"] } } }),
        Category::Todo => state_type("unstarted"),
        Category::InProgress => started("notContainsIgnoreCase"),
        Category::InReview => started("containsIgnoreCase"),
        Category::Done => state_type("completed"),
        Category::Canceled => json!({ "state": { "type": { "in": ["canceled", "duplicate"] } } }),
    }
}

/// The input of `issueCreate`.
pub fn create_input(
    new: &NewTask,
    team_id: &str,
    parent_id: Option<String>,
    account: &str,
) -> CapResult<Value> {
    let mut input = Map::new();
    input.insert("teamId".into(), json!(team_id));
    input.insert("title".into(), json!(new.title.trim()));
    if !new.description.is_empty() {
        input.insert("description".into(), json!(new.description));
    }
    if let Some(status) = &new.status {
        input.insert("stateId".into(), json!(status));
    }
    if new.priority != Priority::None {
        input.insert("priority".into(), json!(new.priority.number()));
    }
    if let Some(project) = &new.project {
        input.insert(
            "projectId".into(),
            json!(scoped_id(project, PROJECT, account, "project")?),
        );
    }
    if !new.labels.is_empty() {
        input.insert("labelIds".into(), json!(label_ids(&new.labels, account)?));
    }
    match new.assignees.as_slice() {
        [] => {}
        [one] => {
            input.insert("assigneeId".into(), json!(one));
        }
        _ => return Err(CapError::invalid("assignees")),
    }
    if let Some(parent) = parent_id {
        input.insert("parentId".into(), json!(parent));
    }
    Ok(Value::Object(input))
}

/// The input of `issueUpdate`: only what the patch names. `parent` has been turned into a Linear uuid already.
pub fn update_input(patch: &Patch, parent: Change<String>, account: &str) -> CapResult<Value> {
    let mut input = Map::new();
    if let Some(title) = &patch.title {
        if title.trim().is_empty() {
            return Err(CapError::invalid("title"));
        }
        input.insert("title".into(), json!(title.trim()));
    }
    match &patch.description {
        Change::Keep => {}
        Change::Set(text) => {
            input.insert("description".into(), json!(text));
        }
        Change::Clear => {
            input.insert("description".into(), json!(""));
        }
    }
    if let Some(status) = &patch.status {
        input.insert("stateId".into(), json!(status));
    }
    if let Some(priority) = patch.priority {
        input.insert("priority".into(), json!(priority.number()));
    }
    match &patch.project {
        Change::Keep => {}
        Change::Set(project) => {
            input.insert(
                "projectId".into(),
                json!(scoped_id(project, PROJECT, account, "project")?),
            );
        }
        Change::Clear => {
            input.insert("projectId".into(), Value::Null);
        }
    }
    if let Some(labels) = &patch.labels {
        input.insert("labelIds".into(), json!(label_ids(labels, account)?));
    }
    match patch.assignees.as_deref() {
        None => {}
        Some([]) => {
            input.insert("assigneeId".into(), Value::Null);
        }
        Some([one]) => {
            input.insert("assigneeId".into(), json!(one));
        }
        Some(_) => return Err(CapError::invalid("assignees")),
    }
    match parent {
        Change::Keep => {}
        Change::Set(id) => {
            input.insert("parentId".into(), json!(id));
        }
        Change::Clear => {
            input.insert("parentId".into(), Value::Null);
        }
    }
    Ok(Value::Object(input))
}

fn label_ids(labels: &[Ref], account: &str) -> CapResult<Vec<String>> {
    labels
        .iter()
        .map(|l| scoped_id(l, LABEL, account, "labels"))
        .collect()
}

// Paging through Linear.

/// Every node of a connection, page after page. `pointer` is where the connection sits in `data`. A connection that
/// is `null` is a thing that does not exist.
pub fn collect(
    client: &Client,
    query: &str,
    variables: Value,
    pointer: &str,
) -> CapResult<Vec<Value>> {
    let mut nodes = vec![];
    let mut after = Value::Null;
    // A ceiling, so a service that never stops saying "more" cannot keep this loop alive.
    for _ in 0..40 {
        let mut vars = variables.clone();
        vars["first"] = json!(PAGE_MAX);
        vars["after"] = after;
        let data = client.run(&with_issue_fields_if_used(query), vars)?;
        let connection = data
            .pointer(pointer)
            .filter(|c| !c.is_null())
            .ok_or_else(|| CapError::not_found(pointer))?;
        nodes.extend(connection["nodes"].as_array().cloned().unwrap_or_default());
        match next_cursor(connection) {
            Some(cursor) => after = json!(cursor),
            None => return Ok(nodes),
        }
    }
    Ok(nodes)
}

fn with_issue_fields_if_used(query: &str) -> String {
    if query.contains("...IssueFields") {
        with_issue_fields(query)
    } else {
        query.to_string()
    }
}

/// The time of the most recent change in the workspace: where `subscribe` starts from. A workspace with nothing
/// in it starts from the epoch.
pub fn latest_update(client: &Client, team: Option<&str>) -> CapResult<String> {
    let filter = team.map_or(Value::Null, |t| json!({ "team": { "key": { "eq": t } } }));
    let data = client.run(types::LATEST, json!({ "filter": filter }))?;
    Ok(data["issues"]["nodes"][0]["updatedAt"]
        .as_str()
        .unwrap_or("1970-01-01T00:00:00.000Z")
        .to_string())
}

/// The tasks changed after `watermark`, oldest change first, and the time of the newest one. A task made after the
/// watermark is `created`, any other is `updated`.
pub fn changes_since(
    client: &Client,
    account: &str,
    team: Option<&str>,
    watermark: &str,
) -> CapResult<(Vec<Event>, Option<String>)> {
    let since = millis(watermark)?;
    let mut filter = vec![json!({ "updatedAt": { "gt": watermark } })];
    if let Some(team) = team {
        filter.push(json!({ "team": { "key": { "eq": team } } }));
    }
    let nodes = collect(
        client,
        types::LIST,
        json!({ "filter": { "and": filter }, "orderBy": "updatedAt" }),
        "/issues",
    )?;
    let mut tasks = nodes
        .iter()
        .map(|n| task_from(account, n))
        .collect::<CapResult<Vec<_>>>()?;
    tasks.sort_by(|a, b| (a.updated_at, &a.key).cmp(&(b.updated_at, &b.key)));
    let newest = tasks.last().map(|t| t.version.clone());
    let events = tasks
        .into_iter()
        .map(|task| Event {
            kind: if task.created_at > since {
                EventKind::Created
            } else {
                EventKind::Updated
            },
            task,
            activity: None,
        })
        .collect();
    Ok((events, newest))
}
