pub(super) const STEPS: &[&str] = &[
    // 1: the tasks, their links, and the log.
    "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
     CREATE TABLE tasks (
         id INTEGER PRIMARY KEY,
         number INTEGER NOT NULL UNIQUE,
         title TEXT NOT NULL,
         description TEXT NOT NULL DEFAULT '',
         status TEXT NOT NULL,
         priority INTEGER NOT NULL DEFAULT 0,
         assignee_kind TEXT,
         assignee_name TEXT,
         project TEXT,
         parent INTEGER REFERENCES tasks(id),
         created_at INTEGER NOT NULL,
         updated_at INTEGER NOT NULL
     );
     CREATE INDEX tasks_status ON tasks (status);
     CREATE INDEX tasks_updated ON tasks (updated_at DESC, id DESC);
     CREATE INDEX tasks_parent ON tasks (parent);
     CREATE TABLE task_labels (
         task INTEGER NOT NULL REFERENCES tasks(id),
         label TEXT NOT NULL,
         PRIMARY KEY (task, label)
     );
     CREATE INDEX task_labels_label ON task_labels (label);
     CREATE TABLE session_links (
         task INTEGER NOT NULL REFERENCES tasks(id),
         session_id TEXT NOT NULL,
         title TEXT NOT NULL,
         agent TEXT NOT NULL,
         PRIMARY KEY (task, session_id)
     );
     CREATE INDEX session_links_session ON session_links (session_id);
     CREATE TABLE pr_links (
         task INTEGER NOT NULL REFERENCES tasks(id),
         number INTEGER NOT NULL,
         repo TEXT NOT NULL,
         PRIMARY KEY (task, number)
     );
     CREATE INDEX pr_links_number ON pr_links (number);
     CREATE TABLE activity (
         id INTEGER PRIMARY KEY AUTOINCREMENT,
         task INTEGER NOT NULL REFERENCES tasks(id),
         at INTEGER NOT NULL,
         actor TEXT NOT NULL,
         data TEXT NOT NULL
     );
     CREATE INDEX activity_task ON activity (task, id);",
];

/// The schema version this build writes.
pub const CURRENT: usize = STEPS.len();
