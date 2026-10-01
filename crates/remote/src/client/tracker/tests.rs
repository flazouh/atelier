use std::collections::HashMap;

use atelier_tracker::{Event, Priority, Status, Task, TaskId};

use super::changes;

fn task(id: &str, at: i64) -> Task {
    Task {
        id: TaskId(id.into()),
        key: id.into(),
        title: id.into(),
        description: String::new(),
        status: Status::Todo,
        priority: Priority::None,
        assignee: None,
        labels: Vec::new(),
        project: None,
        parent: None,
        sessions: Vec::new(),
        prs: Vec::new(),
        created_at: 0,
        updated_at: at,
    }
}

#[test]
fn a_poll_tells_what_is_new_and_what_changed_once() {
    let mut seen = HashMap::from([(TaskId("a".into()), 1), (TaskId("b".into()), 1)]);
    let events = changes(&mut seen, &[task("a", 1), task("b", 2), task("c", 1)]);
    assert_eq!(events, vec![Event::Updated(task("b", 2)), Event::Created(task("c", 1))]);
    assert_eq!(changes(&mut seen, &[task("a", 1), task("b", 2), task("c", 1)]), vec![]);
}
