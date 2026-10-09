use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc::Sender},
};

use atelier_capabilities::{
    Actor, AuthKind, CapError, CapResult, Capabilities, Limits, Operation, Ref, StopFlag,
    Subscription,
    tasks::{
        Activity, Category, Change, Comment, Event, EventKind, Label, NewTask, Page, Patch,
        Project, Query, Status, Task, TasksProvider,
    },
};
use serde_json::{Value, json};

use super::subscribe;
use crate::{
    errors, marker,
    options::Options,
    runner::{Call, Gh, GhCli, Reply},
    scope::{Scope, next_cursor},
    wire,
};

/// Who is told of a change, and what the poller already knows of the repository.
#[derive(Default)]
pub(super) struct Hub {
    pub subscribers: Vec<(Sender<Event>, StopFlag)>,
    /// The `updated_at` of each issue the poller has seen, by number.
    pub known: HashMap<u64, String>,
    /// The highest issue number seen. A number above it is a new issue.
    pub newest: u64,
    pub etag: Option<String>,
    /// Whether a poller thread is alive.
    pub running: bool,
}

/// The issues of one repository, as `tasks`. A task's reference is `tasks:github:<owner>.<repo>:<number>`.
///
/// What GitHub cannot say is left out, not faked. Projects v2, parents, links and the move to another provider
/// (`export`, `import`) are not offered; `projects` is a core call, so it answers with an empty list. Who made a
/// change is always the account of `gh`: the `by` actor reaches GitHub only in a comment, as a hidden line.
pub struct GithubIssues {
    gh: Arc<dyn Gh>,
    pub(super) scope: Arc<Scope>,
    pub(super) hub: Arc<Mutex<Hub>>,
}

impl GithubIssues {
    /// `repo` is `owner/repo`.
    pub fn new(gh: Arc<dyn Gh>, repo: &str, options: Options) -> CapResult<Self> {
        Ok(Self {
            gh,
            scope: Arc::new(Scope::new(repo, options)?),
            hub: Arc::default(),
        })
    }

    /// Through the `gh` on the path, with the default label names.
    pub fn through_gh(repo: &str) -> CapResult<Self> {
        Self::new(Arc::new(GhCli::default()), repo, Options::default())
    }

    /// Sends a call. A failure or an error status is the error; a 304 is a reply.
    fn send(&self, call: Call, what: &str) -> CapResult<Reply> {
        let reply = self.gh.send(&call).map_err(errors::from_failure)?;
        if reply.status >= 400 {
            return Err(errors::from_reply(&reply, what));
        }
        Ok(reply)
    }

    fn json(reply: &Reply) -> CapResult<Value> {
        serde_json::from_str(&reply.body).map_err(|error| CapError::Provider {
            code: "unexpected".into(),
            message: format!("GitHub sent something that is not JSON: {error}"),
        })
    }

    /// The issue as it is now, with its JSON. A pull request is not a task: it is not found.
    fn fetch(&self, number: u64) -> CapResult<(wire::Issue, Value)> {
        let what = format!("issue #{number}");
        let reply = self.send(Call::get(self.scope.issue_path(number)), &what)?;
        let (issue, raw) = Scope::read_issue(Self::json(&reply)?)?;
        if issue.pull_request.is_some() {
            return Err(CapError::not_found(what));
        }
        Ok((issue, raw))
    }

    fn task_of(&self, raw: Value) -> CapResult<Task> {
        let (issue, raw) = Scope::read_issue(raw)?;
        self.scope.task(&issue, raw)
    }

    fn labels_to_names(&self, refs: &[Ref]) -> CapResult<Vec<String>> {
        refs.iter().map(|r| self.scope.label_name(r)).collect()
    }

    /// Tells the subscribers of a change this provider made itself, in order. The poller would see it only as a
    /// snapshot, perhaps merged with the next change.
    fn announce(&self, kind: EventKind, task: &Task) {
        if let Ok(mut hub) = self.hub.lock()
            && hub.running
        {
            subscribe::note(&mut hub, task);
            subscribe::tell(
                &mut hub,
                &Event {
                    kind,
                    task: task.clone(),
                    activity: None,
                },
            );
        }
    }
}

fn names_differ(a: &[String], b: &[String]) -> bool {
    let key = |names: &[String]| {
        let mut v: Vec<String> = names.iter().map(|n| n.to_ascii_lowercase()).collect();
        v.sort();
        v.dedup();
        v
    };
    key(a) != key(b)
}

impl TasksProvider for GithubIssues {
    fn provider(&self) -> &str {
        "github"
    }

    fn account(&self) -> &str {
        &self.scope.account
    }

    fn capabilities(&self) -> Capabilities {
        let mut operations = Capabilities::CORE.to_vec();
        operations.push(Operation::Statuses);
        Capabilities {
            operations,
            features: Vec::new(),
            limits: Limits {
                page_max: Some(100),
                per_minute: None,
            },
            auth: vec![AuthKind::Oauth, AuthKind::Token],
        }
    }

    fn whoami(&self) -> CapResult<Actor> {
        let reply = self.send(Call::get("/user"), "the signed-in user")?;
        let user: wire::User =
            serde_json::from_value(Self::json(&reply)?).map_err(|e| CapError::Provider {
                code: "unexpected".into(),
                message: e.to_string(),
            })?;
        Ok(Scope::actor(&user))
    }

    fn list(&self, query: &Query) -> CapResult<Page<Task>> {
        if query.project.is_some() {
            return Err(CapError::unsupported("filter by project"));
        }
        if query.linked_to.is_some() {
            return Err(CapError::unsupported("filter by link"));
        }
        let path = match &query.cursor {
            Some(cursor) => self.scope.cursor_path(cursor)?,
            None => self.scope.list_path(query)?,
        };
        let reply = self.send(Call::get(&path), "the issues")?;
        let body = Self::json(&reply)?;
        let rows = if path.starts_with("/search/") {
            body["items"].clone()
        } else {
            body
        };
        let mut items = Vec::new();
        for raw in rows.as_array().cloned().unwrap_or_default() {
            let (issue, raw) = Scope::read_issue(raw)?;
            if issue.pull_request.is_some() {
                continue;
            }
            let task = self.scope.task(&issue, raw)?;
            // GitHub cannot filter on a missing label, so a status that is not a plain open or closed one is checked
            // here. A page can therefore hold fewer than `limit` tasks and still have a next page.
            if query.status.is_empty() || query.status.contains(&task.status.category) {
                items.push(task);
            }
        }
        Ok(Page {
            items,
            next_cursor: next_cursor(reply.header("link")),
        })
    }

    fn get(&self, task: &Ref) -> CapResult<Task> {
        let (issue, raw) = self.fetch(self.scope.number_of(task)?)?;
        self.scope.task(&issue, raw)
    }

    fn create(&self, new: &NewTask, _by: &Actor) -> CapResult<Task> {
        if new.title.trim().is_empty() {
            return Err(CapError::invalid("title"));
        }
        if new.project.is_some() {
            return Err(CapError::unsupported("projects"));
        }
        if new.parent.is_some() {
            return Err(CapError::unsupported("subtasks"));
        }
        let category = match &new.status {
            Some(id) => self.scope.category_of_id(id)?,
            None => Category::Todo,
        };
        let change = self.scope.change_for(category);
        let mut labels = self.labels_to_names(&new.labels)?;
        labels.extend(change.label.clone());
        labels.extend(
            self.scope
                .options
                .priority_labels
                .of(new.priority)
                .map(str::to_string),
        );
        let created = self.send(
            Call::post(
                self.scope.issues_path(),
                json!({
                    "title": new.title,
                    "body": new.description,
                    "labels": labels,
                    "assignees": new.assignees,
                }),
            ),
            "the repository",
        )?;
        let mut raw = Self::json(&created)?;
        if change.state == "closed" {
            // An issue is born open. A closed one is made in a second call.
            let number = raw["number"].as_u64().unwrap_or_default();
            let closed = self.send(
                Call::patch(
                    self.scope.issue_path(number),
                    json!({ "state": change.state, "state_reason": change.reason }),
                ),
                "the new issue",
            )?;
            raw = Self::json(&closed)?;
        }
        let task = self.task_of(raw)?;
        self.announce(EventKind::Created, &task);
        Ok(task)
    }

    fn update(&self, task: &Ref, patch: &Patch, version: &str, _by: &Actor) -> CapResult<Task> {
        if matches!(patch.project, Change::Set(_)) {
            return Err(CapError::unsupported("projects"));
        }
        if matches!(patch.parent, Change::Set(_)) {
            return Err(CapError::unsupported("subtasks"));
        }
        let number = self.scope.number_of(task)?;
        let (issue, raw) = self.fetch(number)?;
        let current = self.scope.task(&issue, raw)?;
        // GitHub has no `If-Match` on a write, so this re-read is the check. A change between the read and the write
        // is still lost; the window is one round trip.
        if current.version != version {
            return Err(CapError::Conflict {
                current: serde_json::to_value(&current).unwrap_or_default(),
            });
        }
        let mut body = serde_json::Map::new();
        if let Some(title) = patch.title.as_ref().filter(|t| **t != issue.title) {
            if title.trim().is_empty() {
                return Err(CapError::invalid("title"));
            }
            body.insert("title".into(), json!(title));
        }
        let wanted_body = match &patch.description {
            Change::Keep => None,
            Change::Set(text) => Some(text.as_str()),
            Change::Clear => Some(""),
        };
        if let Some(text) = wanted_body.filter(|t| *t != issue.body.as_deref().unwrap_or_default())
        {
            body.insert("body".into(), json!(text));
        }
        if let Some(logins) = &patch.assignees {
            let now: Vec<String> = issue.assignees.iter().map(|a| a.login.clone()).collect();
            if names_differ(logins, &now) {
                body.insert("assignees".into(), json!(logins));
            }
        }
        // Labels are sent as one whole set. The labels that carry a status or a priority are kept unless the patch
        // changes that status or priority, because a task does not list them and a labels patch must not drop them.
        let have: Vec<String> = issue.labels.iter().map(|l| l.name.clone()).collect();
        let mut names = have.clone();
        if let Some(refs) = &patch.labels {
            let wanted = self.labels_to_names(refs)?;
            names.retain(|n| self.scope.is_control_label(n));
            names.extend(
                wanted
                    .into_iter()
                    .filter(|n| !self.scope.is_control_label(n)),
            );
        }
        if let Some(priority) = patch.priority {
            names.retain(|n| !self.scope.is_priority_label(n));
            names.extend(
                self.scope
                    .options
                    .priority_labels
                    .of(priority)
                    .map(str::to_string),
            );
        }
        if let Some(id) = &patch.status {
            let category = self.scope.category_of_id(id)?;
            let change = self.scope.change_for(category);
            names.retain(|n| self.scope.status_of_label(n).is_none());
            names.extend(change.label);
            if change.state != issue.state
                || (change.state == "closed"
                    && Some(change.reason) != issue.state_reason.as_deref())
            {
                body.insert("state".into(), json!(change.state));
                body.insert("state_reason".into(), json!(change.reason));
            }
        }
        if names_differ(&names, &have) {
            body.insert("labels".into(), json!(names));
        }
        if body.is_empty() {
            return Ok(current);
        }
        let reply = self.send(
            Call::patch(self.scope.issue_path(number), Value::Object(body)),
            &format!("issue #{number}"),
        )?;
        let updated = self.task_of(Self::json(&reply)?)?;
        self.announce(EventKind::Updated, &updated);
        Ok(updated)
    }

    fn comment(&self, task: &Ref, body: &str, by: &Actor) -> CapResult<Comment> {
        if body.trim().is_empty() {
            return Err(CapError::invalid("body"));
        }
        let number = self.scope.number_of(task)?;
        let reply = self.send(
            Call::post(
                format!("{}/comments", self.scope.issue_path(number)),
                json!({ "body": marker::with_actor(body, by) }),
            ),
            &format!("issue #{number}"),
        )?;
        let raw = Self::json(&reply)?;
        let wire: wire::Comment =
            serde_json::from_value(raw.clone()).map_err(|e| CapError::Provider {
                code: "unexpected".into(),
                message: e.to_string(),
            })?;
        self.scope.comment(number, &wire, raw)
    }

    fn activity(&self, task: &Ref, cursor: Option<&str>) -> CapResult<Page<Activity>> {
        let number = self.scope.number_of(task)?;
        let mut items = Vec::new();
        let path = match cursor {
            Some(cursor) => self.scope.cursor_path(cursor)?,
            None => {
                // The first page opens with the creation, which the timeline does not hold.
                let (issue, _) = self.fetch(number)?;
                items.push(self.scope.created_activity(&issue)?);
                format!("{}/timeline?per_page=50", self.scope.issue_path(number))
            }
        };
        let reply = self.send(Call::get(path), &format!("issue #{number}"))?;
        let events: Vec<wire::TimelineEvent> = serde_json::from_value(Self::json(&reply)?)
            .map_err(|e| CapError::Provider {
                code: "unexpected".into(),
                message: e.to_string(),
            })?;
        for (i, event) in events.iter().enumerate() {
            if let Some(activity) =
                self.scope
                    .activity(number, event, &format!("t{}", items.len() + i))
            {
                items.push(activity);
            }
        }
        Ok(Page {
            items,
            next_cursor: next_cursor(reply.header("link")),
        })
    }

    fn labels(&self) -> CapResult<Vec<Label>> {
        let mut path = Some(format!(
            "/repos/{}/{}/labels?per_page=100",
            self.scope.owner, self.scope.repo
        ));
        let mut labels = Vec::new();
        // A repository with more than 1000 labels is cut here, so a loop that never ends is not possible.
        for _ in 0..10 {
            let Some(current) = path.take() else { break };
            let reply = self.send(Call::get(current), "the labels")?;
            for raw in Self::json(&reply)?.as_array().cloned().unwrap_or_default() {
                let wire: wire::Label =
                    serde_json::from_value(raw.clone()).map_err(|e| CapError::Provider {
                        code: "unexpected".into(),
                        message: e.to_string(),
                    })?;
                if !self.scope.is_control_label(&wire.name) {
                    labels.push(self.scope.label(&wire, raw));
                }
            }
            path = next_cursor(reply.header("link"))
                .map(|c| self.scope.cursor_path(&c))
                .transpose()?;
        }
        Ok(labels)
    }

    /// GitHub Projects v2 is not read yet. `projects` is a core call, so it answers with none.
    fn projects(&self) -> CapResult<Vec<Project>> {
        Ok(Vec::new())
    }

    fn subscribe(&self) -> CapResult<Subscription<Event>> {
        subscribe::start(&self.gh, &self.scope, &self.hub)
    }

    fn statuses(&self) -> CapResult<Vec<Status>> {
        Ok([
            Category::Backlog,
            Category::Todo,
            Category::InProgress,
            Category::InReview,
            Category::Done,
            Category::Canceled,
        ]
        .into_iter()
        .map(Status::plain)
        .collect())
    }
}
