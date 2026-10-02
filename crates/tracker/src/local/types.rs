/// The labels come in the same row, joined with a unit separator: one lookup on the primary key of
/// `task_labels` per task is cheaper than a pass over the whole table.
pub(super) const TASK_COLUMNS: &str = "id, number, title, description, status, priority, assignee_kind, assignee_name, project, parent, created_at, updated_at,
     (SELECT group_concat(label, char(31)) FROM task_labels l WHERE l.task = tasks.id)";
