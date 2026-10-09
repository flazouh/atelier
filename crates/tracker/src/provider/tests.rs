use std::sync::Arc;

use atelier_capabilities::{
    Actor,
    tasks::{self as v1, Category, Change, TasksProvider, contract},
};

use super::LocalTasks;
use crate::LocalTracker;

fn provider() -> LocalTasks {
    LocalTasks::new(
        Arc::new(LocalTracker::in_memory("lat").unwrap()),
        "atelier",
        Actor::person("alex", "Alex"),
    )
}

#[test]
fn the_local_tracker_passes_the_tasks_contract() {
    contract::run(&|| Box::new(provider()));
}

#[test]
fn a_task_has_the_short_key_in_its_reference_and_the_old_data_in_raw() {
    let p = provider();
    let t = p
        .create(&v1::NewTask::titled("Look"), &Actor::person("alex", "Alex"))
        .unwrap();
    assert_eq!(t.reference.to_string(), "tasks:local:atelier:LAT-1");
    assert_eq!(t.key, "LAT-1");
    assert_eq!(t.raw.as_ref().unwrap()["key"], "LAT-1");
    assert_eq!(
        p.get(&"tasks:local:atelier:lat-1".parse().unwrap())
            .unwrap()
            .key,
        "LAT-1",
        "a key is found in any case"
    );
    assert!(
        p.get(&"tasks:local:other:LAT-1".parse().unwrap()).is_err(),
        "another account is not found"
    );
}

#[test]
fn an_agent_assignee_keeps_its_kind_and_a_second_assignee_is_refused() {
    let p = provider();
    let by = Actor::person("alex", "Alex");
    let t = p
        .create(
            &v1::NewTask {
                assignees: vec!["agent:Claude".into()],
                ..v1::NewTask::titled("Agent")
            },
            &by,
        )
        .unwrap();
    assert_eq!(t.assignees[0].kind, atelier_capabilities::ActorKind::Agent);
    assert_eq!(t.assignees[0].name, "Claude");
    let two = v1::NewTask {
        assignees: vec!["a".into(), "b".into()],
        ..v1::NewTask::titled("Two")
    };
    assert!(matches!(
        p.create(&two, &by),
        Err(atelier_capabilities::CapError::Invalid { .. })
    ));
}

#[test]
fn labels_project_and_parent_move_through_a_patch() {
    let p = provider();
    let by = Actor::person("alex", "Alex");
    let parent = p.create(&v1::NewTask::titled("Parent"), &by).unwrap();
    let child = p.create(&v1::NewTask::titled("Child"), &by).unwrap();
    let label: atelier_capabilities::Ref = "tasks:local:atelier:ui".parse().unwrap();
    let project: atelier_capabilities::Ref = "tasks:local:atelier:app".parse().unwrap();
    let after = p
        .update(
            &child.reference,
            &v1::Patch {
                labels: Some(vec![label.clone()]),
                project: Change::Set(project.clone()),
                parent: Change::Set(parent.reference.clone()),
                ..v1::Patch::default()
            },
            &child.version,
            &by,
        )
        .unwrap();
    assert_eq!(
        (
            after.labels.clone(),
            after.project.clone(),
            after.parent.clone()
        ),
        (
            vec![label.clone()],
            Some(project),
            Some(parent.reference.clone())
        )
    );
    assert_eq!(p.labels().unwrap().len(), 1);
    assert_eq!(p.projects().unwrap().len(), 1);
    let found = p
        .list(&v1::Query {
            labels: vec![label],
            ..v1::Query::default()
        })
        .unwrap();
    assert_eq!(found.items.len(), 1);
    let cleared = p
        .update(
            &child.reference,
            &v1::Patch {
                labels: Some(vec![]),
                parent: Change::Clear,
                project: Change::Clear,
                ..v1::Patch::default()
            },
            &after.version,
            &by,
        )
        .unwrap();
    assert!(cleared.labels.is_empty() && cleared.parent.is_none() && cleared.project.is_none());
}

#[test]
fn the_list_filters_by_category_and_sorts_by_priority() {
    let p = provider();
    let by = Actor::person("alex", "Alex");
    p.create(
        &v1::NewTask {
            priority: v1::Priority::Low,
            ..v1::NewTask::titled("Low")
        },
        &by,
    )
    .unwrap();
    p.create(
        &v1::NewTask {
            priority: v1::Priority::Urgent,
            ..v1::NewTask::titled("Urgent")
        },
        &by,
    )
    .unwrap();
    p.create(
        &v1::NewTask {
            status: Some("done".into()),
            ..v1::NewTask::titled("Done")
        },
        &by,
    )
    .unwrap();
    let open = p
        .list(&v1::Query {
            status: vec![Category::Todo],
            sort: v1::Sort::Priority,
            ..v1::Query::default()
        })
        .unwrap();
    assert_eq!(
        open.items
            .iter()
            .map(|t| t.title.as_str())
            .collect::<Vec<_>>(),
        ["Urgent", "Low"]
    );
}

#[test]
fn tasks_of_a_session_are_found_by_a_session_reference() {
    use crate::{SessionLink, Tracker};
    let tracker = Arc::new(LocalTracker::in_memory("lat").unwrap());
    let p = LocalTasks::new(tracker.clone(), "atelier", Actor::person("alex", "Alex"));
    let a = p
        .create(
            &v1::NewTask::titled("Linked"),
            &Actor::person("alex", "Alex"),
        )
        .unwrap();
    p.create(
        &v1::NewTask::titled("Alone"),
        &Actor::person("alex", "Alex"),
    )
    .unwrap();
    let id = tracker
        .list(&crate::Query::default())
        .unwrap()
        .into_iter()
        .find(|t| t.key == a.key)
        .unwrap()
        .id;
    tracker
        .record(
            &id,
            &crate::Entry::SessionStarted(SessionLink {
                session_id: "s1".into(),
                title: "T".into(),
                agent: "Claude".into(),
            }),
            "Claude",
        )
        .unwrap();
    let found = p
        .list(&v1::Query {
            linked_to: Some("session:atelier:atelier:s1".into()),
            ..v1::Query::default()
        })
        .unwrap();
    assert_eq!(found.items.len(), 1);
    assert_eq!(
        found.items[0].links[0].reference,
        "session:atelier:atelier:s1"
    );
}
