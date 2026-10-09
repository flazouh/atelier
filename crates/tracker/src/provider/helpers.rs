use atelier_capabilities::{
    Actor, ActorKind, CapError, Ref,
    tasks::{self as v1, ActivityKind as V1Kind},
};

use crate::{Activity, ActivityKind, Assignee, Priority, Status, Task, Tracker, TrackerError};

/// The id of an agent's actor. A person's id is the name. The tracker keeps only names, so this tells the two apart.
const AGENT_PREFIX: &str = "agent:";

pub(super) fn category_of(status: Status) -> v1::Category {
    match status {
        Status::Backlog => v1::Category::Backlog,
        Status::Todo => v1::Category::Todo,
        Status::InProgress => v1::Category::InProgress,
        Status::InReview => v1::Category::InReview,
        Status::Done => v1::Category::Done,
        Status::Canceled => v1::Category::Canceled,
    }
}

pub(super) fn status_of(category: v1::Category) -> Status {
    match category {
        v1::Category::Backlog => Status::Backlog,
        v1::Category::Todo => Status::Todo,
        v1::Category::InProgress => Status::InProgress,
        v1::Category::InReview => Status::InReview,
        v1::Category::Done => Status::Done,
        v1::Category::Canceled => Status::Canceled,
    }
}

pub(super) fn priority_to(priority: Priority) -> v1::Priority {
    v1::Priority::from_number(priority.number() as u8).unwrap_or_default()
}

pub(super) fn priority_from(priority: v1::Priority) -> Priority {
    Priority::from_number(i64::from(priority.number()))
}

pub(super) fn actor_of(assignee: &Assignee) -> Actor {
    match assignee {
        Assignee::Person(name) => Actor::person(name, name),
        Assignee::Agent(name) => Actor {
            kind: ActorKind::Agent,
            id: format!("{AGENT_PREFIX}{name}"),
            name: name.clone(),
            on_behalf_of: None,
        },
    }
}

pub(super) fn assignee_of(id: &str) -> Assignee {
    match id.strip_prefix(AGENT_PREFIX) {
        Some(name) => Assignee::Agent(name.to_string()),
        None => Assignee::Person(id.to_string()),
    }
}

/// An actor id as the tracker's assignee name.
pub(super) fn assignee_name(id: &str) -> String {
    assignee_of(id).name().to_string()
}

/// A reference part: the characters a reference allows, and nothing empty.
pub(super) fn account_part(text: &str) -> String {
    let clean: String = text
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    if clean.is_empty() {
        "project".into()
    } else {
        clean
    }
}

pub(super) fn ref_of(account: &str, id: &str) -> Ref {
    Ref {
        capability: "tasks".into(),
        provider: "local".into(),
        account: account.into(),
        id: id.into(),
    }
}

/// Fingerprint of everything a person can change in a task, so two edits in one second still differ.
pub(super) fn version_of(task: &Task) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |text: &str| {
        for byte in text.bytes().chain(std::iter::once(0)) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    feed(&task.title);
    feed(&task.description);
    feed(task.status.as_str());
    feed(&task.priority.number().to_string());
    feed(task.assignee.as_ref().map_or("", Assignee::name));
    task.labels.iter().for_each(|l| feed(l));
    feed(task.project.as_deref().unwrap_or(""));
    feed(
        &task
            .parent
            .as_ref()
            .map_or_else(String::new, |p| p.0.clone()),
    );
    feed(&task.updated_at.to_string());
    format!("{hash:016x}")
}

fn repo_account(repo: &str) -> String {
    account_part(&repo.replace('/', "."))
}

pub(super) fn task_of(account: &str, task: &Task, parent_key: Option<&str>) -> v1::Task {
    let links = task
        .sessions
        .iter()
        .map(|s| v1::TaskLink {
            kind: v1::LinkKind::Session,
            reference: format!("session:atelier:{account}:{}", s.session_id),
        })
        .chain(task.prs.iter().map(|p| v1::TaskLink {
            kind: v1::LinkKind::PullRequest,
            reference: format!("git:github:{}:pull/{}", repo_account(&p.repo), p.number),
        }))
        .collect();
    v1::Task {
        reference: ref_of(account, &task.key),
        key: task.key.clone(),
        title: task.title.clone(),
        description: task.description.clone(),
        status: v1::Status::plain(category_of(task.status)),
        priority: priority_to(task.priority),
        assignees: task.assignee.iter().map(actor_of).collect(),
        labels: task.labels.iter().map(|l| ref_of(account, l)).collect(),
        project: task.project.as_deref().map(|p| ref_of(account, p)),
        parent: parent_key.map(|key| ref_of(account, key)),
        links,
        created_at: task.created_at * 1000,
        updated_at: task.updated_at * 1000,
        version: version_of(task),
        due_at: None,
        estimate: None,
        raw: serde_json::to_value(task).ok(),
    }
}

pub(super) fn activity_of(
    account: &str,
    task_key: &str,
    activity: &Activity,
    by: Actor,
) -> v1::Activity {
    let (kind, detail) = match &activity.kind {
        ActivityKind::Created => (V1Kind::Created, None),
        ActivityKind::StatusChanged { from, to } => (
            V1Kind::StatusChanged,
            Some(serde_json::json!({ "from": from.as_str(), "to": to.as_str() })),
        ),
        ActivityKind::Assigned { to } => (
            V1Kind::Assigned,
            Some(serde_json::json!({ "to": to.as_ref().map(|a| actor_of(a).id) })),
        ),
        ActivityKind::Edited { field } => {
            (V1Kind::Edited, Some(serde_json::json!({ "field": field })))
        }
        ActivityKind::Commented { text } => {
            (V1Kind::Commented, Some(serde_json::json!({ "text": text })))
        }
        ActivityKind::SessionStarted { session } => {
            (V1Kind::SessionStarted, serde_json::to_value(session).ok())
        }
        ActivityKind::PrOpened { pr } => (V1Kind::PrOpened, serde_json::to_value(pr).ok()),
        ActivityKind::PrMerged { pr } => (V1Kind::PrMerged, serde_json::to_value(pr).ok()),
        ActivityKind::Commit { sha, subject } => (
            V1Kind::Commit,
            Some(serde_json::json!({ "sha": sha, "subject": subject })),
        ),
    };
    v1::Activity {
        reference: ref_of(account, &format!("activity-{}", activity.id)),
        task: ref_of(account, task_key),
        at: activity.at * 1000,
        by,
        kind,
        detail,
    }
}

pub(super) fn error_of(error: TrackerError) -> CapError {
    match error {
        TrackerError::NotFound(id) => CapError::not_found(format!("task {id}")),
        TrackerError::Invalid(why) => CapError::Invalid { field: why },
        TrackerError::Storage(why) => CapError::Storage { message: why },
        TrackerError::Unsupported(what) => CapError::Unsupported { feature: what },
    }
}

/// A task as v1, with its parent's key read from the tracker.
pub(super) fn task_with_parent(tracker: &dyn Tracker, account: &str, task: &Task) -> v1::Task {
    let parent = task
        .parent
        .as_ref()
        .and_then(|id| tracker.get(id).ok().flatten());
    task_of(account, task, parent.as_ref().map(|p| p.key.as_str()))
}
