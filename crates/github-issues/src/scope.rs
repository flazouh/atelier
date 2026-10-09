//! The repository one provider serves, and how GitHub's issues read as tasks: references, statuses, priorities,
//! labels, comments and activity. Nothing here calls GitHub.
use atelier_capabilities::{
    Actor, CapError, CapResult, Ref,
    tasks::{
        Activity, ActivityKind, Category, Comment, Label, Priority, Query, Sort, Status, Task,
    },
};
use serde_json::{Value, json};

use crate::{
    marker,
    options::Options,
    time,
    wire::{self, Issue, TimelineEvent},
};

/// What the label of a status or a priority is called on the GitHub side of a patch.
pub struct StatusChange {
    /// `open` or `closed`.
    pub state: &'static str,
    /// `completed`, `not_planned` or `reopened`.
    pub reason: &'static str,
    /// The label that stands for the status, when it has one.
    pub label: Option<String>,
}

pub struct Scope {
    pub owner: String,
    pub repo: String,
    /// `owner.repo`: the account part of a reference. An owner never holds a dot, so the first dot splits it back.
    pub account: String,
    pub options: Options,
}

impl Scope {
    /// `repo` is `owner/repo`.
    pub fn new(repo: &str, options: Options) -> CapResult<Self> {
        let name_ok = |s: &str, extra: &[char]| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || extra.contains(&c))
        };
        let (owner, name) = repo
            .split_once('/')
            .filter(|(o, n)| name_ok(o, &[]) && name_ok(n, &['_', '.']) && *n != "." && *n != "..")
            .ok_or_else(|| CapError::invalid("repo"))?;
        let account = format!("{owner}.{name}");
        Ref::new("tasks", "github", &account, "1").map_err(|_| CapError::invalid("repo"))?;
        Ok(Self {
            owner: owner.into(),
            repo: name.into(),
            account,
            options,
        })
    }

    // ---- references

    pub fn task_ref(&self, number: u64) -> Ref {
        self.ref_with_id(&number.to_string())
    }

    fn ref_with_id(&self, id: &str) -> Ref {
        Ref::new("tasks", "github", &self.account, id).expect("the account was checked in new")
    }

    pub fn label_ref(&self, name: &str) -> Ref {
        self.ref_with_id(&format!("label:{name}"))
    }

    fn is_mine(&self, r: &Ref) -> bool {
        r.capability == "tasks" && r.provider == "github" && r.account == self.account
    }

    /// The issue number of a task reference. A reference of another account, or one that is no number, is not found.
    pub fn number_of(&self, r: &Ref) -> CapResult<u64> {
        self.is_mine(r)
            .then(|| r.id.parse().ok())
            .flatten()
            .ok_or_else(|| CapError::not_found(r.to_string()))
    }

    pub fn label_name(&self, r: &Ref) -> CapResult<String> {
        r.id.strip_prefix("label:")
            .filter(|_| self.is_mine(r))
            .map(str::to_string)
            .ok_or_else(|| CapError::invalid("labels"))
    }

    pub fn issues_path(&self) -> String {
        format!("/repos/{}/{}/issues", self.owner, self.repo)
    }

    pub fn issue_path(&self, number: u64) -> String {
        format!("{}/{number}", self.issues_path())
    }

    // ---- status and priority

    pub fn category(&self, issue: &Issue) -> Category {
        if issue.state == "closed" {
            return if issue.state_reason.as_deref() == Some("not_planned") {
                Category::Canceled
            } else {
                Category::Done
            };
        }
        let has = |name: &str| {
            issue
                .labels
                .iter()
                .any(|l| l.name.eq_ignore_ascii_case(name))
        };
        self.options
            .status_labels
            .all()
            .into_iter()
            .find(|(_, name)| has(name))
            .map_or(Category::Todo, |(category, _)| category)
    }

    pub fn priority(&self, issue: &Issue) -> Priority {
        let has = |name: &str| {
            issue
                .labels
                .iter()
                .any(|l| l.name.eq_ignore_ascii_case(name))
        };
        self.options
            .priority_labels
            .all()
            .into_iter()
            .find(|(_, name)| has(name))
            .map_or(Priority::None, |(priority, _)| priority)
    }

    /// The category a status label stands for, or none when the name is not one.
    pub fn status_of_label(&self, name: &str) -> Option<Category> {
        self.options
            .status_labels
            .all()
            .into_iter()
            .find(|(_, label)| label.eq_ignore_ascii_case(name))
            .map(|(category, _)| category)
    }

    pub fn is_priority_label(&self, name: &str) -> bool {
        self.options
            .priority_labels
            .all()
            .into_iter()
            .any(|(_, label)| label.eq_ignore_ascii_case(name))
    }

    /// A label that carries a status or a priority. A task does not list it as a label: it would say the same twice.
    pub fn is_control_label(&self, name: &str) -> bool {
        self.status_of_label(name).is_some() || self.is_priority_label(name)
    }

    /// The category a status id of this provider names. The ids are the six plain words.
    pub fn category_of_id(&self, id: &str) -> CapResult<Category> {
        [
            Category::Backlog,
            Category::Todo,
            Category::InProgress,
            Category::InReview,
            Category::Done,
            Category::Canceled,
        ]
        .into_iter()
        .find(|c| Status::plain(*c).id == id)
        .ok_or_else(|| CapError::invalid("status"))
    }

    pub fn change_for(&self, category: Category) -> StatusChange {
        let (state, reason) = match category {
            Category::Done => ("closed", "completed"),
            Category::Canceled => ("closed", "not_planned"),
            _ => ("open", "reopened"),
        };
        StatusChange {
            state,
            reason,
            label: self.options.status_labels.of(category).map(str::to_string),
        }
    }

    // ---- entities

    pub fn actor(user: &wire::User) -> Actor {
        Actor::person(&user.login, user.name.as_deref().unwrap_or(&user.login))
    }

    fn ghost() -> Actor {
        Actor::person("ghost", "ghost")
    }

    fn millis(text: &str) -> CapResult<i64> {
        time::parse(text).ok_or_else(|| CapError::Provider {
            code: "unexpected".into(),
            message: format!("`{text}` is not a timestamp"),
        })
    }

    /// The issue as GitHub sent it, read, and the JSON it came from.
    pub fn read_issue(raw: Value) -> CapResult<(Issue, Value)> {
        let issue = serde_json::from_value(raw.clone()).map_err(|error| CapError::Provider {
            code: "unexpected".into(),
            message: format!("an issue could not be read: {error}"),
        })?;
        Ok((issue, raw))
    }

    pub fn task(&self, issue: &Issue, raw: Value) -> CapResult<Task> {
        Ok(Task {
            reference: self.task_ref(issue.number),
            key: format!("#{}", issue.number),
            title: issue.title.clone(),
            description: issue.body.clone().unwrap_or_default(),
            status: Status::plain(self.category(issue)),
            priority: self.priority(issue),
            assignees: issue.assignees.iter().map(Self::actor).collect(),
            labels: issue
                .labels
                .iter()
                .filter(|l| !self.is_control_label(&l.name))
                .map(|l| self.label_ref(&l.name))
                .collect(),
            project: None,
            parent: None,
            links: Vec::new(),
            created_at: Self::millis(&issue.created_at)?,
            updated_at: Self::millis(&issue.updated_at)?,
            // GitHub gives no etag per issue that a write can check, so the time of the last change stands for it.
            version: issue.updated_at.clone(),
            due_at: None,
            estimate: None,
            raw: Some(raw),
        })
    }

    pub fn label(&self, label: &wire::Label, raw: Value) -> Label {
        Label {
            reference: self.label_ref(&label.name),
            name: label.name.clone(),
            color: label.color.clone(),
            raw: Some(raw),
        }
    }

    pub fn comment(&self, number: u64, c: &wire::Comment, raw: Value) -> CapResult<Comment> {
        let (body, written_by) = marker::split(c.body.as_deref().unwrap_or_default());
        Ok(Comment {
            reference: self.comment_ref(number, c.id),
            task: self.task_ref(number),
            author: written_by
                .unwrap_or_else(|| c.user.as_ref().map_or_else(Self::ghost, Self::actor)),
            body,
            created_at: Self::millis(&c.created_at)?,
            updated_at: c.updated_at.as_deref().map(Self::millis).transpose()?,
            raw: Some(raw),
        })
    }

    fn comment_ref(&self, number: u64, id: u64) -> Ref {
        self.ref_with_id(&format!("{number}:c{id}"))
    }

    /// GitHub's timeline has no entry for the creation: the issue itself says who made it and when.
    pub fn created_activity(&self, issue: &Issue) -> CapResult<Activity> {
        Ok(Activity {
            reference: self.ref_with_id(&format!("{}:created", issue.number)),
            task: self.task_ref(issue.number),
            at: Self::millis(&issue.created_at)?,
            by: issue.user.as_ref().map_or_else(Self::ghost, Self::actor),
            kind: ActivityKind::Created,
            detail: None,
        })
    }

    /// One timeline entry as an activity. An entry that says nothing a task screen shows (a mention, a subscription)
    /// is none. `slot` names the entry when GitHub gave it no id.
    pub fn activity(&self, number: u64, e: &TimelineEvent, slot: &str) -> Option<Activity> {
        let at = e.created_at.as_deref().and_then(time::parse)?;
        let by = e
            .actor
            .as_ref()
            .or(e.user.as_ref())
            .map_or_else(Self::ghost, Self::actor);
        let id = e.id.map_or_else(
            || format!("{number}:{slot}"),
            |id| format!("{number}:e{id}"),
        );
        let label = e.label.as_ref().map(|l| l.name.as_str());
        let (kind, detail, by) = match e.event.as_str() {
            "commented" => {
                let (body, written_by) = marker::split(e.body.as_deref().unwrap_or_default());
                let comment = e.id.map(|id| self.comment_ref(number, id).to_string());
                (
                    ActivityKind::Commented,
                    json!({ "comment": comment, "body": body }),
                    written_by.unwrap_or(by),
                )
            }
            "labeled" | "unlabeled" => {
                let name = label?;
                let change = e.event.as_str();
                match self.status_of_label(name) {
                    Some(category) => (
                        ActivityKind::StatusChanged,
                        json!({ "label": name, "change": change, "category": category }),
                        by,
                    ),
                    None => (
                        ActivityKind::Edited,
                        json!({ "label": name, "change": change }),
                        by,
                    ),
                }
            }
            "closed" => {
                let to = if e.state_reason.as_deref() == Some("not_planned") {
                    Category::Canceled
                } else {
                    Category::Done
                };
                (ActivityKind::StatusChanged, json!({ "to": to }), by)
            }
            "reopened" => (
                ActivityKind::StatusChanged,
                json!({ "to": Category::Todo }),
                by,
            ),
            "assigned" | "unassigned" => (
                ActivityKind::Assigned,
                json!({ "change": e.event, "assignee": e.assignee.as_ref().map(|a| &a.login) }),
                by,
            ),
            "renamed" => {
                let rename = e.rename.as_ref()?;
                (
                    ActivityKind::Edited,
                    json!({ "title": { "from": rename.from, "to": rename.to } }),
                    by,
                )
            }
            "referenced" => (
                ActivityKind::Commit,
                json!({ "commit": e.commit_id.as_ref()? }),
                by,
            ),
            "cross-referenced" => {
                let issue = e.source.as_ref()?.issue.as_ref()?;
                issue.pull_request.as_ref()?;
                (
                    ActivityKind::PrOpened,
                    json!({ "number": issue.number, "title": issue.title }),
                    by,
                )
            }
            _ => return None,
        };
        Some(Activity {
            reference: self.ref_with_id(&id),
            task: self.task_ref(number),
            at,
            by,
            kind,
            detail: Some(detail),
        })
    }

    // ---- paths

    /// Where a list starts: the issues, or a search when the query has text to find.
    pub fn list_path(&self, query: &Query) -> CapResult<String> {
        let limit = query.limit.unwrap_or(50).clamp(1, 100);
        let labels = query
            .labels
            .iter()
            .map(|r| self.label_name(r))
            .collect::<CapResult<Vec<_>>>()?;
        let closed = |c: &Category| matches!(c, Category::Done | Category::Canceled);
        let open = |c: &Category| !closed(c);
        let state = match (
            query.status.iter().any(open),
            query.status.iter().any(closed),
        ) {
            (true, false) => "open",
            (false, true) => "closed",
            _ => "all",
        };
        let sort = match query.sort {
            Sort::Created => "created",
            Sort::Updated => "updated",
            Sort::Priority => return Err(CapError::unsupported("sort by priority")),
        };
        if let Some(text) = &query.text {
            let mut q = format!("repo:{}/{} is:issue {text}", self.owner, self.repo);
            if state != "all" {
                q.push_str(&format!(" is:{state}"));
            }
            for name in &labels {
                q.push_str(&format!(" label:\"{name}\""));
            }
            if let Some(login) = &query.assignee {
                q.push_str(&format!(" assignee:{login}"));
            }
            return Ok(format!(
                "/search/issues?q={}&sort={sort}&order=desc&per_page={limit}",
                encode(&q)
            ));
        }
        let mut path = format!(
            "{}?state={state}&sort={sort}&direction=desc&per_page={limit}",
            self.issues_path()
        );
        if !labels.is_empty() {
            let joined = labels
                .iter()
                .map(|n| encode(n))
                .collect::<Vec<_>>()
                .join(",");
            path.push_str(&format!("&labels={joined}"));
        }
        if let Some(login) = &query.assignee {
            path.push_str(&format!("&assignee={}", encode(login)));
        }
        Ok(path)
    }

    /// A cursor is the path GitHub's `Link: rel="next"` gave. It is used only for this repository.
    pub fn cursor_path(&self, cursor: &str) -> CapResult<String> {
        let mine = cursor.starts_with(&format!("/repos/{}/{}/", self.owner, self.repo))
            || cursor.starts_with("/repositories/")
            || cursor.starts_with("/search/issues?");
        if mine && !cursor.contains("..") {
            Ok(cursor.to_string())
        } else {
            Err(CapError::invalid("cursor"))
        }
    }
}

/// The path and query of a `Link: rel="next"` header, without the host.
pub fn next_cursor(link: Option<&str>) -> Option<String> {
    link?
        .split(',')
        .find(|part| part.contains("rel=\"next\""))
        .and_then(|part| part.split('<').nth(1)?.split('>').next())
        .map(|url| match url.split_once("://") {
            Some((_, rest)) => rest.find('/').map_or("", |i| &rest[i..]),
            None => url,
        })
        .map(str::to_string)
}

/// Percent-encodes everything but the characters a URL query takes as they are.
pub fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests;
