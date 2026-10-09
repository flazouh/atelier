use atelier_capabilities::{
    Actor, ActorKind, CapError, Ref,
    tasks::{ActivityKind, Category, Change, NewTask, Patch, Priority, Query, Sort},
};
use serde_json::{Value, json};

use super::fixture;
use crate::provider::helpers::*;

const ACCOUNT: &str = "acme";

fn first_issue() -> Value {
    fixture("list")["data"]["issues"]["nodes"][0].clone()
}

fn reference(text: &str) -> Ref {
    text.parse().unwrap()
}

#[test]
fn a_state_type_becomes_a_category() {
    let cases = [
        ("backlog", "Backlog", Some(Category::Backlog)),
        ("triage", "Triage", Some(Category::Backlog)),
        ("unstarted", "Todo", Some(Category::Todo)),
        ("started", "In Progress", Some(Category::InProgress)),
        ("started", "In Review", Some(Category::InReview)),
        ("started", "Code REVIEW", Some(Category::InReview)),
        ("completed", "Done", Some(Category::Done)),
        ("canceled", "Canceled", Some(Category::Canceled)),
        ("duplicate", "Duplicate", Some(Category::Canceled)),
        ("unstarted", "Ready for review", Some(Category::Todo)),
        ("something new", "X", None),
    ];
    for (kind, name, want) in cases {
        assert_eq!(category_of(kind, name), want, "{kind} named {name}");
    }
}

#[test]
fn an_issue_becomes_a_task() {
    let raw = first_issue();
    let task = task_from(ACCOUNT, &raw).unwrap();
    assert_eq!(task.reference.to_string(), "tasks:linear:acme:ACM-1");
    assert_eq!(task.key, "ACM-1");
    assert_eq!(task.status.name, "Done");
    assert_eq!(task.status.category, Category::Done);
    assert_eq!(task.priority, Priority::None);
    assert_eq!(task.assignees.len(), 1);
    assert_eq!(task.assignees[0].name, "Person 1");
    assert_eq!(task.assignees[0].kind, ActorKind::Person);
    assert_eq!(task.labels.len(), 1);
    assert!(
        task.labels[0]
            .to_string()
            .starts_with("tasks:linear:acme:label:")
    );
    assert!(
        task.project
            .unwrap()
            .to_string()
            .starts_with("tasks:linear:acme:project:")
    );
    assert_eq!(task.parent.unwrap().to_string(), "tasks:linear:acme:ACM-2");
    assert_eq!(
        task.created_at, 1_791_502_272_196,
        "milliseconds since the epoch"
    );
    assert_eq!(task.updated_at, 1_791_507_792_521);
    assert_eq!(
        task.version, "2026-10-09T01:03:12.521Z",
        "the version is updatedAt as Linear wrote it"
    );
    assert_eq!(
        task.raw.as_ref(),
        Some(&raw),
        "the provider's own JSON is kept"
    );
}

#[test]
fn a_task_with_nothing_optional_has_empty_fields() {
    let raw = fixture("list")["data"]["issues"]["nodes"][2].clone();
    let task = task_from(ACCOUNT, &raw).unwrap();
    assert_eq!(task.status.category, Category::Backlog);
    assert!(task.assignees.is_empty() && task.labels.is_empty());
    assert!(task.parent.is_none() && task.due_at.is_none() && task.estimate.is_none());
}

#[test]
fn priority_numbers_keep_linears_numbering() {
    let expected = [
        Priority::None,
        Priority::Urgent,
        Priority::High,
        Priority::Medium,
        Priority::Low,
    ];
    for (number, priority) in expected.into_iter().enumerate() {
        let mut raw = first_issue();
        raw["priority"] = json!(number);
        assert_eq!(task_from(ACCOUNT, &raw).unwrap().priority, priority);
    }
    let mut raw = first_issue();
    raw["priority"] = json!(9);
    assert!(matches!(
        task_from(ACCOUNT, &raw),
        Err(CapError::Provider { .. })
    ));
}

#[test]
fn a_due_date_is_midnight_utc_and_an_estimate_stays_a_number() {
    let mut raw = first_issue();
    raw["dueDate"] = json!("2026-10-12");
    raw["estimate"] = json!(3.0);
    let task = task_from(ACCOUNT, &raw).unwrap();
    assert_eq!(task.due_at, Some(1_791_763_200_000));
    assert_eq!(task.estimate, Some(3.0));
}

#[test]
fn an_issue_that_lacks_a_field_is_a_provider_error_not_a_panic() {
    let mut raw = first_issue();
    raw.as_object_mut().unwrap().remove("state");
    match task_from(ACCOUNT, &raw) {
        Err(CapError::Provider { code, .. }) => assert_eq!(code, "unexpected_shape"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_page_carries_linears_cursor_only_while_there_is_more() {
    let page = task_page(ACCOUNT, &fixture("list")["data"]["issues"]).unwrap();
    assert_eq!(page.items.len(), 3);
    assert_eq!(
        page.next_cursor.as_deref(),
        fixture("list")["data"]["issues"]["pageInfo"]["endCursor"].as_str()
    );
    let mut last = fixture("list")["data"]["issues"].clone();
    last["pageInfo"] = json!({ "hasNextPage": false, "endCursor": "x" });
    assert_eq!(task_page(ACCOUNT, &last).unwrap().next_cursor, None);
}

#[test]
fn labels_projects_and_statuses_map() {
    let labels = fixture("labels");
    let label = label_from(ACCOUNT, &labels["data"]["issueLabels"]["nodes"][0]).unwrap();
    assert_eq!(label.name, "Label 2");
    assert_eq!(label.color.as_deref(), Some("#8B5CF6"));
    assert!(
        label
            .reference
            .to_string()
            .starts_with("tasks:linear:acme:label:")
    );

    let projects = fixture("projects");
    let project = project_from(ACCOUNT, &projects["data"]["projects"]["nodes"][0]).unwrap();
    assert_eq!(
        (project.key.as_str(), project.name.as_str()),
        ("slug3", "Project 3")
    );

    let states = fixture("states");
    let all: Vec<_> = states["data"]["workflowStates"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| status_from(s).unwrap())
        .collect();
    let by_name = |name: &str| all.iter().find(|s| s.name == name).unwrap().category;
    assert_eq!(by_name("In Review"), Category::InReview);
    assert_eq!(by_name("In Progress"), Category::InProgress);
    assert_eq!(by_name("Todo"), Category::Todo);
    assert_eq!(by_name("Duplicate"), Category::Canceled);
}

#[test]
fn a_comment_keeps_its_author_and_time() {
    let task = reference("tasks:linear:acme:ACM-1");
    let raw = fixture("comments")["data"]["issue"]["comments"]["nodes"][0].clone();
    let comment = comment_from(ACCOUNT, &task, &raw).unwrap();
    assert_eq!(comment.author.name, "Person 1");
    assert!(
        comment
            .reference
            .to_string()
            .starts_with("tasks:linear:acme:comment:")
    );
    assert_eq!(comment.task, task);
    assert_eq!(comment.created_at, 1_791_507_792_548);
}

#[test]
fn an_agents_comment_says_so_in_its_last_line_and_a_persons_does_not() {
    let agent = Actor::agent("a1", "Claude", "u1");
    assert_eq!(
        comment_body("Looks right.", &agent),
        "Looks right.\n\n_Written by the agent Claude._"
    );
    assert_eq!(
        comment_body("Looks right.", &Actor::person("u1", "Alex")),
        "Looks right."
    );
}

#[test]
fn activity_lists_creation_history_and_comments_oldest_first() {
    let task = reference("tasks:linear:acme:ACM-1");
    let issue = fixture("get")["data"]["issue"].clone();
    let history = fixture("history")["data"]["issue"]["history"]["nodes"]
        .as_array()
        .unwrap()
        .clone();
    let comments = fixture("comments")["data"]["issue"]["comments"]["nodes"]
        .as_array()
        .unwrap()
        .clone();
    let page = activity_page(ACCOUNT, &task, &issue, &history, &comments, None).unwrap();
    assert_eq!(page.items.len(), 1 + history.len() + comments.len());
    assert_eq!(page.items[0].kind, ActivityKind::Created);
    let times: Vec<_> = page.items.iter().map(|a| a.at).collect();
    assert!(
        times.windows(2).all(|w| w[0] <= w[1]),
        "oldest first: {times:?}"
    );
    let kinds: Vec<_> = page.items.iter().map(|a| a.kind).collect();
    assert!(kinds.contains(&ActivityKind::StatusChanged));
    assert!(kinds.contains(&ActivityKind::Edited));
    assert!(kinds.contains(&ActivityKind::Commented));
    let by_app = page
        .items
        .iter()
        .find(|a| a.by.id.starts_with("app:"))
        .expect("an app made a change");
    assert_eq!(by_app.by.kind, ActorKind::Agent, "an app is not a person");
    assert!(page.next_cursor.is_none());
}

#[test]
fn a_long_activity_pages_without_repeats_or_gaps() {
    let task = reference("tasks:linear:acme:ACM-1");
    let issue = fixture("get")["data"]["issue"].clone();
    let comments: Vec<Value> = (0..120)
        .map(|i| {
            let mut c = fixture("comments")["data"]["issue"]["comments"]["nodes"][0].clone();
            c["id"] = json!(format!("c{i:03}"));
            c["createdAt"] = json!(format!("2026-10-09T02:00:{:02}.{:03}Z", i / 60, i % 60));
            c
        })
        .collect();
    let mut seen = vec![];
    let mut cursor: Option<String> = None;
    for _ in 0..10 {
        let page =
            activity_page(ACCOUNT, &task, &issue, &[], &comments, cursor.as_deref()).unwrap();
        assert!(page.items.len() <= 50);
        seen.extend(page.items.into_iter().map(|a| a.reference.to_string()));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen.len(), 121);
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 121, "no entry twice");
    assert!(matches!(
        activity_page(ACCOUNT, &task, &issue, &[], &[], Some("x")),
        Err(CapError::Invalid { .. })
    ));
}

#[test]
fn only_a_task_of_this_workspace_is_a_task_key() {
    assert_eq!(
        task_key(&reference("tasks:linear:acme:ACM-1"), ACCOUNT).unwrap(),
        "ACM-1"
    );
    for other in [
        "tasks:github:acme:ACM-1",
        "tasks:linear:other:ACM-1",
        "tasks:linear:acme:label:abc",
    ] {
        assert!(
            matches!(
                task_key(&reference(other), ACCOUNT),
                Err(CapError::NotFound { .. })
            ),
            "{other}"
        );
    }
}

#[test]
fn a_filter_says_each_thing_the_query_asks() {
    let none = Query::default();
    assert_eq!(issue_filter(&none, None, ACCOUNT).unwrap(), Value::Null);
    assert_eq!(
        issue_filter(&none, Some("ACM"), ACCOUNT).unwrap(),
        json!({ "team": { "key": { "eq": "ACM" } } })
    );
    let q = Query {
        status: vec![Category::InProgress, Category::InReview, Category::Done],
        assignee: Some("u1".into()),
        project: Some(reference("tasks:linear:acme:project:p1")),
        labels: vec![
            reference("tasks:linear:acme:label:l1"),
            reference("tasks:linear:acme:label:l2"),
        ],
        text: Some("login".into()),
        ..Query::default()
    };
    let filter = issue_filter(&q, None, ACCOUNT).unwrap();
    let all = filter["and"].as_array().unwrap();
    assert_eq!(all.len(), 6);
    assert_eq!(
        all[0],
        json!({ "or": [
            { "state": { "type": { "eq": "started" }, "name": { "notContainsIgnoreCase": "review" } } },
            { "state": { "type": { "eq": "started" }, "name": { "containsIgnoreCase": "review" } } },
            { "state": { "type": { "eq": "completed" } } },
        ] })
    );
    assert_eq!(all[1], json!({ "assignee": { "id": { "eq": "u1" } } }));
    assert_eq!(all[2], json!({ "project": { "id": { "eq": "p1" } } }));
    assert_eq!(
        all[3],
        json!({ "labels": { "some": { "id": { "eq": "l1" } } } })
    );
    assert_eq!(
        all[4],
        json!({ "labels": { "some": { "id": { "eq": "l2" } } } }),
        "every label must be there"
    );
    assert_eq!(
        all[5],
        json!({ "searchableContent": { "contains": "login" } })
    );
}

#[test]
fn what_linear_cannot_do_is_unsupported_and_what_is_not_ours_is_invalid() {
    let linked = Query {
        linked_to: Some("x".into()),
        ..Query::default()
    };
    assert!(matches!(
        issue_filter(&linked, None, ACCOUNT),
        Err(CapError::Unsupported { .. })
    ));
    let by_priority = Query {
        sort: Sort::Priority,
        ..Query::default()
    };
    assert!(matches!(
        order_by(&by_priority),
        Err(CapError::Unsupported { .. })
    ));
    assert_eq!(order_by(&Query::default()).unwrap(), "updatedAt");
    let foreign = Query {
        project: Some(reference("tasks:github:acme:project:p1")),
        ..Query::default()
    };
    assert!(matches!(
        issue_filter(&foreign, None, ACCOUNT),
        Err(CapError::Invalid { field }) if field == "project"
    ));
}

#[test]
fn a_new_task_becomes_the_input_of_issue_create() {
    let new = NewTask {
        title: "  Fix the login ".into(),
        description: "It fails.".into(),
        status: Some("s1".into()),
        priority: Priority::High,
        project: Some(reference("tasks:linear:acme:project:p1")),
        labels: vec![reference("tasks:linear:acme:label:l1")],
        assignees: vec!["u1".into()],
        parent: None,
    };
    assert_eq!(
        create_input(&new, "team1", Some("uuid-parent".into()), ACCOUNT).unwrap(),
        json!({
            "teamId": "team1", "title": "Fix the login", "description": "It fails.", "stateId": "s1",
            "priority": 2, "projectId": "p1", "labelIds": ["l1"], "assigneeId": "u1", "parentId": "uuid-parent",
        })
    );
    assert_eq!(
        create_input(&NewTask::titled("Plain"), "team1", None, ACCOUNT).unwrap(),
        json!({ "teamId": "team1", "title": "Plain" }),
        "a plain task sends no empty fields"
    );
    let two = NewTask {
        assignees: vec!["a".into(), "b".into()],
        ..NewTask::titled("T")
    };
    assert!(
        matches!(
            create_input(&two, "t", None, ACCOUNT),
            Err(CapError::Invalid { .. })
        ),
        "Linear has one assignee"
    );
}

#[test]
fn a_patch_sends_only_what_it_names_and_null_clears() {
    assert_eq!(
        update_input(&Patch::default(), Change::Keep, ACCOUNT).unwrap(),
        json!({})
    );
    let patch = Patch {
        title: Some("New".into()),
        description: Change::Clear,
        status: Some("s2".into()),
        priority: Some(Priority::Urgent),
        project: Change::Clear,
        labels: Some(vec![]),
        assignees: Some(vec![]),
        parent: Change::Keep,
    };
    assert_eq!(
        update_input(&patch, Change::Clear, ACCOUNT).unwrap(),
        json!({
            "title": "New", "description": "", "stateId": "s2", "priority": 1, "projectId": null,
            "labelIds": [], "assigneeId": null, "parentId": null,
        })
    );
    let set = Patch {
        assignees: Some(vec!["u9".into()]),
        ..Patch::default()
    };
    assert_eq!(
        update_input(&set, Change::Set("uuid".into()), ACCOUNT).unwrap(),
        json!({ "assigneeId": "u9", "parentId": "uuid" })
    );
    let blank = Patch {
        title: Some("  ".into()),
        ..Patch::default()
    };
    assert!(matches!(
        update_input(&blank, Change::Keep, ACCOUNT),
        Err(CapError::Invalid { .. })
    ));
}
