//! The local tracker: one SQLite database per project, in the app's data folder. Every call blocks, so the
//! app makes none on the UI thread. One connection behind a lock; the database is in WAL mode, so a write
//! is one small append and does not wait for the disk to settle.
use std::{
    collections::HashMap,
    path::Path,
    sync::{
        Mutex,
        mpsc::{Receiver, Sender, channel},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, Transaction, params, params_from_iter, types::Value};

use crate::{
    Activity, ActivityKind, Assignee, Entry, Event, NewTask, Patch, PrLink, Priority, ProjectKey, Query, SessionLink, Status,
    Task, TaskId, Tracker, TrackerError, TrackerResult, prefix_for,
};

mod migrations;

impl From<rusqlite::Error> for TrackerError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl From<serde_json::Error> for TrackerError {
    fn from(error: serde_json::Error) -> Self {
        Self::Storage(format!("a stored entry does not read: {error}"))
    }
}

pub struct LocalTracker {
    conn: Mutex<Connection>,
    prefix: String,
    clock: Box<dyn Fn() -> i64 + Send + Sync>,
    subscribers: Mutex<Vec<Sender<Event>>>,
}

impl LocalTracker {
    /// Opens (or makes) the database at `path`, and brings it up to the current schema. `prefix` is the
    /// prefix of short ids for a new database; an existing one keeps the prefix it was made with, so a
    /// project that is renamed keeps its ids.
    pub fn open(path: &Path, prefix: &str) -> TrackerResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| TrackerError::Storage(format!("cannot make {}: {e}", parent.display())))?;
        }
        Self::with_connection(Connection::open(path)?, prefix)
    }

    /// The database of a project, in the app's data folder (`<data>/tracker/<name>.sqlite`). The prefix
    /// comes from the project's name.
    pub fn open_project(data_dir: &Path, project: &ProjectKey, name: &str) -> TrackerResult<Self> {
        Self::open(&project.path_in(data_dir), &prefix_for(name))
    }

    /// A database that lives in memory only. For tests and stories.
    pub fn in_memory(prefix: &str) -> TrackerResult<Self> {
        Self::with_connection(Connection::open_in_memory()?, prefix)
    }

    fn with_connection(mut conn: Connection, prefix: &str) -> TrackerResult<Self> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "foreign_keys", true)?;
        // WAL is refused for an in-memory database, which is fine: the answer is "memory".
        let _ = conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()));
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        migrations::run(&mut conn)?;
        let stored: Option<String> = conn.query_row("SELECT value FROM meta WHERE key = 'prefix'", [], |r| r.get(0)).optional()?;
        let prefix = match stored {
            Some(stored) => stored,
            None => {
                let prefix = prefix.to_uppercase();
                conn.execute("INSERT INTO meta (key, value) VALUES ('prefix', ?1)", [&prefix])?;
                prefix
            }
        };
        Ok(Self {
            conn: Mutex::new(conn),
            prefix,
            clock: Box::new(|| SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)),
            subscribers: Mutex::new(Vec::new()),
        })
    }

    /// Replaces the clock, so a test controls the times in the log.
    pub fn with_clock(mut self, clock: impl Fn() -> i64 + Send + Sync + 'static) -> Self {
        self.clock = Box::new(clock);
        self
    }

    /// The prefix of short ids: "LAT".
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    fn now(&self) -> i64 {
        (self.clock)()
    }

    fn conn(&self) -> TrackerResult<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| TrackerError::Storage("the task store lock was poisoned".into()))
    }

    fn emit(&self, events: Vec<Event>) {
        if let Ok(mut subscribers) = self.subscribers.lock() {
            for event in events {
                subscribers.retain(|s| s.send(event.clone()).is_ok());
            }
        }
    }
}

fn row_id(id: &TaskId) -> TrackerResult<i64> {
    id.0.parse().map_err(|_| TrackerError::NotFound(id.clone()))
}

fn assignee_parts(assignee: &Option<Assignee>) -> (Option<&'static str>, Option<String>) {
    match assignee {
        Some(Assignee::Person(name)) => (Some("person"), Some(name.clone())),
        Some(Assignee::Agent(name)) => (Some("agent"), Some(name.clone())),
        None => (None, None),
    }
}

/// The labels come in the same row, joined with a unit separator: one lookup on the primary key of
/// `task_labels` per task is cheaper than a pass over the whole table.
const TASK_COLUMNS: &str = "id, number, title, description, status, priority, assignee_kind, assignee_name, project, parent, created_at, updated_at,
     (SELECT group_concat(label, char(31)) FROM task_labels l WHERE l.task = tasks.id)";

/// A task row with no links yet.
fn read_task(prefix: &str, row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
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
            let mut labels: Vec<String> = joined.map(|j| j.split('\u{1f}').map(str::to_string).collect()).unwrap_or_default();
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

/// The session and pull request links of some tasks: those `scope` names (`WHERE ...` over `task`), or all.
struct Links {
    sessions: HashMap<i64, Vec<SessionLink>>,
    prs: HashMap<i64, Vec<PrLink>>,
}

impl Links {
    fn load(conn: &Connection, scope: &str, binds: &[Value]) -> rusqlite::Result<Links> {
        let mut sessions: HashMap<i64, Vec<SessionLink>> = HashMap::new();
        let mut stmt = conn.prepare_cached(&format!("SELECT task, session_id, title, agent FROM session_links{scope} ORDER BY rowid"))?;
        for row in stmt.query_map(params_from_iter(binds), |r| {
            Ok((r.get::<_, i64>(0)?, SessionLink { session_id: r.get(1)?, title: r.get(2)?, agent: r.get(3)? }))
        })? {
            let (task, link) = row?;
            sessions.entry(task).or_default().push(link);
        }
        let mut prs: HashMap<i64, Vec<PrLink>> = HashMap::new();
        let mut stmt = conn.prepare_cached(&format!("SELECT task, number, repo FROM pr_links{scope} ORDER BY number"))?;
        for row in stmt.query_map(params_from_iter(binds), |r| {
            Ok((r.get::<_, i64>(0)?, PrLink { number: r.get::<_, i64>(1)? as u64, repo: r.get(2)? }))
        })? {
            let (task, link) = row?;
            prs.entry(task).or_default().push(link);
        }
        Ok(Links { sessions, prs })
    }

    fn fill(&mut self, task: &mut Task) {
        if let Ok(id) = task.id.0.parse::<i64>() {
            task.sessions = self.sessions.remove(&id).unwrap_or_default();
            task.prs = self.prs.remove(&id).unwrap_or_default();
        }
    }
}

fn load_one(conn: &Connection, prefix: &str, id: i64) -> TrackerResult<Option<Task>> {
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

fn like_pattern(text: &str) -> String {
    let escaped = text.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{escaped}%")
}

fn log(tx: &Transaction<'_>, task: i64, at: i64, by: &str, kind: &ActivityKind) -> TrackerResult<Activity> {
    let data = serde_json::to_string(kind)?;
    tx.prepare_cached("INSERT INTO activity (task, at, actor, data) VALUES (?1, ?2, ?3, ?4)")?.execute(params![task, at, by, data])?;
    Ok(Activity { id: tx.last_insert_rowid(), task: TaskId(task.to_string()), at, by: by.to_string(), kind: kind.clone() })
}

fn insert(tx: &Transaction<'_>, new: &NewTask, at: i64, by: &str) -> TrackerResult<(i64, Activity)> {
    if new.title.trim().is_empty() {
        return Err(TrackerError::Invalid("a task needs a title".into()));
    }
    let parent = match &new.parent {
        Some(parent) => {
            let parent = row_id(parent)?;
            let exists: bool = tx.query_row("SELECT EXISTS (SELECT 1 FROM tasks WHERE id = ?1)", [parent], |r| r.get(0))?;
            if !exists {
                return Err(TrackerError::NotFound(TaskId(parent.to_string())));
            }
            Some(parent)
        }
        None => None,
    };
    let number: i64 = tx.query_row("SELECT COALESCE(MAX(number), 0) + 1 FROM tasks", [], |r| r.get(0))?;
    let (kind, name) = assignee_parts(&new.assignee);
    tx.prepare_cached(
        "INSERT INTO tasks (number, title, description, status, priority, assignee_kind, assignee_name, project, parent, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
    )?
    .execute(params![number, new.title.trim(), new.description, new.status.as_str(), new.priority.number(), kind, name, new.project, parent, at])?;
    let id = tx.last_insert_rowid();
    for label in &new.labels {
        tx.prepare_cached("INSERT OR IGNORE INTO task_labels (task, label) VALUES (?1, ?2)")?.execute(params![id, label.trim()])?;
    }
    let activity = log(tx, id, at, by, &ActivityKind::Created)?;
    Ok((id, activity))
}

impl Tracker for LocalTracker {
    fn name(&self) -> &str {
        "local"
    }

    fn list(&self, query: &Query) -> TrackerResult<Vec<Task>> {
        let mut conditions: Vec<String> = Vec::new();
        let mut binds: Vec<Value> = Vec::new();
        if !query.statuses.is_empty() {
            let marks = vec!["?"; query.statuses.len()].join(", ");
            conditions.push(format!("status IN ({marks})"));
            binds.extend(query.statuses.iter().map(|s| Value::Text(s.as_str().into())));
        }
        if let Some(name) = &query.assignee {
            conditions.push("assignee_name = ?".into());
            binds.push(Value::Text(name.clone()));
        }
        if let Some(label) = &query.label {
            conditions.push("EXISTS (SELECT 1 FROM task_labels l WHERE l.task = tasks.id AND l.label = ?)".into());
            binds.push(Value::Text(label.clone()));
        }
        if let Some(priority) = query.priority {
            conditions.push("priority = ?".into());
            binds.push(Value::Integer(priority.number()));
        }
        if let Some(text) = query.text.as_deref().filter(|t| !t.trim().is_empty()) {
            conditions.push("(title LIKE ? ESCAPE '\\' OR (? || '-' || number) LIKE ? ESCAPE '\\')".into());
            let pattern = like_pattern(text.trim());
            binds.extend([Value::Text(pattern.clone()), Value::Text(self.prefix.clone()), Value::Text(pattern)]);
        }
        if let Some(parent) = &query.parent {
            conditions.push("parent = ?".into());
            binds.push(Value::Integer(row_id(parent)?));
        }
        let filter = if conditions.is_empty() { String::new() } else { format!(" WHERE {}", conditions.join(" AND ")) };
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(&format!("SELECT {TASK_COLUMNS} FROM tasks{filter} ORDER BY updated_at DESC, id DESC"))?;
        let mut tasks = stmt.query_map(params_from_iter(&binds), |r| read_task(&self.prefix, r))?.collect::<Result<Vec<_>, _>>()?;
        let scope = if conditions.is_empty() { String::new() } else { format!(" WHERE task IN (SELECT id FROM tasks{filter})") };
        let mut links = Links::load(&conn, &scope, &binds)?;
        for task in &mut tasks {
            links.fill(task);
        }
        Ok(tasks)
    }

    fn get(&self, id: &TaskId) -> TrackerResult<Option<Task>> {
        let Ok(row) = row_id(id) else { return Ok(None) };
        load_one(&*self.conn()?, &self.prefix, row)
    }

    fn create(&self, new: &NewTask, by: &str) -> TrackerResult<Task> {
        let mut created = self.create_many(std::slice::from_ref(new), by)?;
        created.pop().ok_or_else(|| TrackerError::Storage("no task was made".into()))
    }

    fn create_many(&self, new: &[NewTask], by: &str) -> TrackerResult<Vec<Task>> {
        let at = self.now();
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut ids = Vec::with_capacity(new.len());
        for task in new {
            ids.push(insert(&tx, task, at, by)?.0);
        }
        tx.commit()?;
        let mut tasks = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(task) = load_one(&conn, &self.prefix, id)? {
                tasks.push(task);
            }
        }
        drop(conn);
        self.emit(tasks.iter().cloned().map(Event::Created).collect());
        Ok(tasks)
    }

    fn update(&self, id: &TaskId, patch: &Patch, by: &str) -> TrackerResult<Task> {
        let row = row_id(id)?;
        let at = self.now();
        let mut conn = self.conn()?;
        let before = load_one(&conn, &self.prefix, row)?.ok_or_else(|| TrackerError::NotFound(id.clone()))?;
        let mut after = before.clone();
        let mut entries: Vec<ActivityKind> = Vec::new();
        if let Some(title) = &patch.title {
            if title.trim().is_empty() {
                return Err(TrackerError::Invalid("a task needs a title".into()));
            }
            if title.trim() != before.title {
                after.title = title.trim().to_string();
                entries.push(ActivityKind::Edited { field: "title".into() });
            }
        }
        if let Some(description) = &patch.description
            && *description != before.description
        {
            after.description = description.clone();
            entries.push(ActivityKind::Edited { field: "description".into() });
        }
        if let Some(status) = patch.status
            && status != before.status
        {
            after.status = status;
            entries.push(ActivityKind::StatusChanged { from: before.status, to: status });
        }
        if let Some(priority) = patch.priority
            && priority != before.priority
        {
            after.priority = priority;
            entries.push(ActivityKind::Edited { field: "priority".into() });
        }
        if let Some(assignee) = &patch.assignee
            && *assignee != before.assignee
        {
            after.assignee = assignee.clone();
            entries.push(ActivityKind::Assigned { to: assignee.clone() });
        }
        if let Some(project) = &patch.project
            && *project != before.project
        {
            after.project = project.clone();
            entries.push(ActivityKind::Edited { field: "project".into() });
        }
        if let Some(parent) = &patch.parent
            && *parent != before.parent
        {
            if let Some(parent) = parent {
                let mut walk = Some(row_id(parent)?);
                let mut steps = 0;
                while let Some(at_row) = walk {
                    if at_row == row {
                        return Err(TrackerError::Invalid("a task cannot be its own ancestor".into()));
                    }
                    let up: Option<Option<i64>> = conn.query_row("SELECT parent FROM tasks WHERE id = ?1", [at_row], |r| r.get(0)).optional()?;
                    let Some(up) = up else { return Err(TrackerError::NotFound(parent.clone())) };
                    walk = up;
                    steps += 1;
                    if steps > 64 {
                        return Err(TrackerError::Invalid("the sub-tasks nest too deep".into()));
                    }
                }
            }
            after.parent = parent.clone();
            entries.push(ActivityKind::Edited { field: "parent".into() });
        }
        let mut labels = before.labels.clone();
        for label in &patch.add_labels {
            let label = label.trim().to_string();
            if !label.is_empty() && !labels.contains(&label) {
                labels.push(label);
            }
        }
        labels.retain(|l| !patch.remove_labels.contains(l));
        labels.sort();
        if labels != before.labels {
            after.labels = labels;
            entries.push(ActivityKind::Edited { field: "labels".into() });
        }
        if entries.is_empty() {
            return Ok(before);
        }
        after.updated_at = at;
        let tx = conn.transaction()?;
        let (kind, name) = assignee_parts(&after.assignee);
        tx.prepare_cached(
            "UPDATE tasks SET title = ?2, description = ?3, status = ?4, priority = ?5, assignee_kind = ?6, assignee_name = ?7,
                              project = ?8, parent = ?9, updated_at = ?10 WHERE id = ?1",
        )?
        .execute(params![
            row,
            after.title,
            after.description,
            after.status.as_str(),
            after.priority.number(),
            kind,
            name,
            after.project,
            after.parent.as_ref().map(row_id).transpose()?,
            at
        ])?;
        if after.labels != before.labels {
            for label in before.labels.iter().filter(|l| !after.labels.contains(l)) {
                tx.prepare_cached("DELETE FROM task_labels WHERE task = ?1 AND label = ?2")?.execute(params![row, label])?;
            }
            for label in after.labels.iter().filter(|l| !before.labels.contains(l)) {
                tx.prepare_cached("INSERT OR IGNORE INTO task_labels (task, label) VALUES (?1, ?2)")?.execute(params![row, label])?;
            }
        }
        let mut events = vec![Event::Updated(after.clone())];
        for entry in &entries {
            events.push(Event::Activity(log(&tx, row, at, by, entry)?));
        }
        tx.commit()?;
        drop(conn);
        self.emit(events);
        Ok(after)
    }

    fn record(&self, id: &TaskId, entry: &Entry, by: &str) -> TrackerResult<Activity> {
        let row = row_id(id)?;
        let at = self.now();
        let kind = match entry {
            Entry::Comment(text) => {
                if text.trim().is_empty() {
                    return Err(TrackerError::Invalid("a comment needs words".into()));
                }
                ActivityKind::Commented { text: text.trim().to_string() }
            }
            Entry::SessionStarted(session) => ActivityKind::SessionStarted { session: session.clone() },
            Entry::PrOpened(pr) => ActivityKind::PrOpened { pr: pr.clone() },
            Entry::PrMerged(pr) => ActivityKind::PrMerged { pr: pr.clone() },
        };
        let mut conn = self.conn()?;
        let exists: bool = conn.query_row("SELECT EXISTS (SELECT 1 FROM tasks WHERE id = ?1)", [row], |r| r.get(0))?;
        if !exists {
            return Err(TrackerError::NotFound(id.clone()));
        }
        let tx = conn.transaction()?;
        match entry {
            Entry::SessionStarted(session) => {
                tx.prepare_cached("INSERT OR IGNORE INTO session_links (task, session_id, title, agent) VALUES (?1, ?2, ?3, ?4)")?
                    .execute(params![row, session.session_id, session.title, session.agent])?;
            }
            Entry::PrOpened(pr) | Entry::PrMerged(pr) => {
                tx.prepare_cached("INSERT OR IGNORE INTO pr_links (task, number, repo) VALUES (?1, ?2, ?3)")?
                    .execute(params![row, pr.number as i64, pr.repo])?;
            }
            Entry::Comment(_) => {}
        }
        tx.prepare_cached("UPDATE tasks SET updated_at = ?2 WHERE id = ?1")?.execute(params![row, at])?;
        let activity = log(&tx, row, at, by, &kind)?;
        tx.commit()?;
        let task = load_one(&conn, &self.prefix, row)?;
        drop(conn);
        let mut events = Vec::new();
        events.extend(task.map(Event::Updated));
        events.push(Event::Activity(activity.clone()));
        self.emit(events);
        Ok(activity)
    }

    fn activity(&self, id: &TaskId) -> TrackerResult<Vec<Activity>> {
        let row = row_id(id)?;
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached("SELECT id, at, actor, data FROM activity WHERE task = ?1 ORDER BY id")?;
        let rows = stmt.query_map([row], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?;
        let mut out = Vec::new();
        for entry in rows {
            let (entry_id, at, by, data) = entry?;
            out.push(Activity { id: entry_id, task: id.clone(), at, by, kind: serde_json::from_str(&data)? });
        }
        Ok(out)
    }

    fn tasks_of_session(&self, session_id: &str) -> TrackerResult<Vec<TaskId>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached("SELECT DISTINCT task FROM session_links WHERE session_id = ?1 ORDER BY task")?;
        Ok(stmt.query_map([session_id], |r| r.get::<_, i64>(0))?.map(|r| r.map(|n| TaskId(n.to_string()))).collect::<Result<_, _>>()?)
    }

    fn tasks_of_pr(&self, number: u64) -> TrackerResult<Vec<TaskId>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached("SELECT DISTINCT task FROM pr_links WHERE number = ?1 ORDER BY task")?;
        Ok(stmt.query_map([number as i64], |r| r.get::<_, i64>(0))?.map(|r| r.map(|n| TaskId(n.to_string()))).collect::<Result<_, _>>()?)
    }

    fn labels(&self) -> TrackerResult<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached("SELECT DISTINCT label FROM task_labels ORDER BY label")?;
        Ok(stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?)
    }

    fn subscribe(&self) -> Receiver<Event> {
        let (send, receive) = channel();
        if let Ok(mut subscribers) = self.subscribers.lock() {
            subscribers.push(send);
        }
        receive
    }
}

#[cfg(test)]
mod tests;
