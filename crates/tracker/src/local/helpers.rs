use rusqlite::{Connection, OptionalExtension, Transaction, params, types::Value};

use super::structs::Links;
use super::types::TASK_COLUMNS;
use crate::{
    Activity, ActivityKind, Assignee, NewTask, Priority, Status, Task, TaskId, TrackerError,
    TrackerResult,
};

pub(super) fn row_id(id: &TaskId) -> TrackerResult<i64> {
    id.0.parse().map_err(|_| TrackerError::NotFound(id.clone()))
}

pub(super) fn assignee_parts(
    assignee: &Option<Assignee>,
) -> (Option<&'static str>, Option<String>) {
    match assignee {
        Some(Assignee::Person(name)) => (Some("person"), Some(name.clone())),
        Some(Assignee::Agent(name)) => (Some("agent"), Some(name.clone())),
        None => (None, None),
    }
}

/// A task row with no links yet.
pub(super) fn read_task(prefix: &str, row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let id: i64 = row.get(0)?;
    let number: i64 = row.get(1)?;
    let status: String = row.get(4)?;
    let kind: Option<String> = row.get(6)?;
    let name: Option<String> = row.get(7)?;
    let parent: Option<i64> = row.get(9)?;
    Ok(Task {
        id: TaskId(id.to_string()),
        key: format!("{prefix}-{number}"),
        title: row.get(2)?,
        description: row.get(3)?,
        status: Status::parse(&status).unwrap_or(Status::Backlog),
        priority: Priority::from_number(row.get(5)?),
        assignee: match (kind.as_deref(), name) {
            (Some("agent"), Some(name)) => Some(Assignee::Agent(name)),
            (Some(_), Some(name)) => Some(Assignee::Person(name)),
            _ => None,
        },
        labels: {
            let joined: Option<String> = row.get(12)?;
            let mut labels: Vec<String> = joined
                .map(|j| j.split('\u{1f}').map(str::to_string).collect())
                .unwrap_or_default();
            labels.sort();
            labels
        },
        project: row.get(8)?,
        parent: parent.map(|p| TaskId(p.to_string())),
        sessions: Vec::new(),
        prs: Vec::new(),
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

pub(super) fn load_one(conn: &Connection, prefix: &str, id: i64) -> TrackerResult<Option<Task>> {
    let task = conn
        .prepare_cached(&format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1"))?
        .query_row([id], |r| read_task(prefix, r))
        .optional()?;
    Ok(match task {
        Some(mut task) => {
            Links::load(conn, " WHERE task = ?1", &[Value::Integer(id)])?.fill(&mut task);
            Some(task)
        }
        None => None,
    })
}

pub(super) fn like_pattern(text: &str) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

pub(super) fn log(
    tx: &Transaction<'_>,
    task: i64,
    at: i64,
    by: &str,
    kind: &ActivityKind,
) -> TrackerResult<Activity> {
    let data = serde_json::to_string(kind)?;
    tx.prepare_cached("INSERT INTO activity (task, at, actor, data) VALUES (?1, ?2, ?3, ?4)")?
        .execute(params![task, at, by, data])?;
    Ok(Activity {
        id: tx.last_insert_rowid(),
        task: TaskId(task.to_string()),
        at,
        by: by.to_string(),
        kind: kind.clone(),
    })
}

pub(super) fn insert(
    tx: &Transaction<'_>,
    new: &NewTask,
    at: i64,
    by: &str,
) -> TrackerResult<(i64, Activity)> {
    if new.title.trim().is_empty() {
        return Err(TrackerError::Invalid("a task needs a title".into()));
    }
    let parent = match &new.parent {
        Some(parent) => {
            let parent = row_id(parent)?;
            let exists: bool = tx.query_row(
                "SELECT EXISTS (SELECT 1 FROM tasks WHERE id = ?1)",
                [parent],
                |r| r.get(0),
            )?;
            if !exists {
                return Err(TrackerError::NotFound(TaskId(parent.to_string())));
            }
            Some(parent)
        }
        None => None,
    };
    let number: i64 = tx.query_row("SELECT COALESCE(MAX(number), 0) + 1 FROM tasks", [], |r| {
        r.get(0)
    })?;
    let (kind, name) = assignee_parts(&new.assignee);
    tx.prepare_cached(
        "INSERT INTO tasks (number, title, description, status, priority, assignee_kind, assignee_name, project, parent, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
    )?
    .execute(params![number, new.title.trim(), new.description, new.status.as_str(), new.priority.number(), kind, name, new.project, parent, at])?;
    let id = tx.last_insert_rowid();
    for label in &new.labels {
        tx.prepare_cached("INSERT OR IGNORE INTO task_labels (task, label) VALUES (?1, ?2)")?
            .execute(params![id, label.trim()])?;
    }
    let activity = log(tx, id, at, by, &ActivityKind::Created)?;
    Ok((id, activity))
}
