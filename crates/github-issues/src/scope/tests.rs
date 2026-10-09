use atelier_capabilities::{
    Actor, CapError, Ref,
    tasks::{ActivityKind, Category, Priority, Query, Sort},
};
use serde_json::json;

use super::{Scope, encode, next_cursor};
use crate::{
    marker,
    options::{Options, StatusLabels},
    wire::{Issue, TimelineEvent},
};

fn scope() -> Scope {
    Scope::new("acme/widgets", Options::default()).unwrap()
}

fn issue(state: &str, reason: Option<&str>, labels: &[&str]) -> Issue {
    serde_json::from_value(json!({
        "number": 7, "title": "T", "state": state, "state_reason": reason,
        "labels": labels.iter().map(|n| json!({ "name": n })).collect::<Vec<_>>(),
        "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-02T00:00:00Z",
    }))
    .unwrap()
}

#[test]
fn a_status_comes_from_the_state_the_reason_and_the_labels() {
    let s = scope();
    let cases = [
        (("open", None, vec![]), Category::Todo),
        (("open", None, vec!["bug"]), Category::Todo),
        (
            ("open", None, vec!["status:in-progress"]),
            Category::InProgress,
        ),
        (("open", None, vec!["Status:In-Review"]), Category::InReview),
        (("open", None, vec!["status:backlog"]), Category::Backlog),
        (
            ("open", None, vec!["status:in-progress", "status:in-review"]),
            Category::InReview,
        ),
        (("closed", Some("completed"), vec![]), Category::Done),
        (("closed", None, vec![]), Category::Done),
        (("closed", Some("not_planned"), vec![]), Category::Canceled),
        // A status label on a closed issue does not undo the close.
        (
            ("closed", Some("completed"), vec!["status:in-progress"]),
            Category::Done,
        ),
    ];
    for ((state, reason, labels), want) in cases {
        assert_eq!(
            s.category(&issue(state, reason, &labels)),
            want,
            "{state} {reason:?} {labels:?}"
        );
    }
}

#[test]
fn a_priority_comes_from_its_label_and_the_names_are_options() {
    let s = scope();
    assert_eq!(
        s.priority(&issue("open", None, &["priority:high"])),
        Priority::High
    );
    assert_eq!(
        s.priority(&issue("open", None, &["priority:low", "priority:urgent"])),
        Priority::Urgent
    );
    assert_eq!(s.priority(&issue("open", None, &["p1"])), Priority::None);
    let custom = Scope::new(
        "acme/widgets",
        Options {
            status_labels: StatusLabels {
                in_progress: "doing".into(),
                ..StatusLabels::default()
            },
            priority_labels: crate::PriorityLabels {
                urgent: "P0".into(),
                ..Default::default()
            },
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(
        custom.category(&issue("open", None, &["doing"])),
        Category::InProgress
    );
    assert_eq!(
        custom.category(&issue("open", None, &["status:in-progress"])),
        Category::Todo
    );
    assert_eq!(
        custom.priority(&issue("open", None, &["p0"])),
        Priority::Urgent
    );
}

#[test]
fn the_labels_of_a_task_leave_out_the_ones_that_carry_a_status_or_a_priority() {
    let s = scope();
    let i = issue(
        "open",
        None,
        &[
            "bug",
            "status:in-review",
            "priority:high",
            "good first issue",
        ],
    );
    let task = s.task(&i, json!({})).unwrap();
    let names: Vec<_> = task
        .labels
        .iter()
        .map(|r| s.label_name(r).unwrap())
        .collect();
    assert_eq!(names, ["bug", "good first issue"]);
    assert_eq!(
        (task.key.as_str(), task.version.as_str()),
        ("#7", "2026-01-02T00:00:00Z")
    );
    assert_eq!(task.created_at, 1_767_225_600_000);
}

#[test]
fn a_repository_name_is_the_account_and_the_reference_splits_back() {
    let s = Scope::new("acme/my.widgets_v2", Options::default()).unwrap();
    assert_eq!(s.account, "acme.my.widgets_v2");
    let r = s.task_ref(42);
    assert_eq!(r.to_string(), "tasks:github:acme.my.widgets_v2:42");
    let back: Ref = r.to_string().parse().unwrap();
    assert_eq!(s.number_of(&back).unwrap(), 42);
    // The first dot splits owner from repository, because an owner never has one.
    let (owner, repo) = back.account.split_once('.').unwrap();
    assert_eq!((owner, repo), ("acme", "my.widgets_v2"));
    for other in [
        "tasks:github:acme.other:42",
        "tasks:linear:acme.my.widgets_v2:42",
        "tasks:github:acme.my.widgets_v2:abc",
    ] {
        assert!(
            matches!(
                s.number_of(&other.parse().unwrap()),
                Err(CapError::NotFound { .. })
            ),
            "{other}"
        );
    }
}

#[test]
fn a_repository_that_is_not_owner_slash_name_is_invalid() {
    for bad in [
        "",
        "acme",
        "acme/",
        "/widgets",
        "a.b/widgets",
        "acme/wid gets",
        "acme/a/b",
        "acme/..",
        "acme/wid:gets",
    ] {
        assert_eq!(
            Scope::new(bad, Options::default()).err(),
            Some(CapError::invalid("repo")),
            "{bad:?}"
        );
    }
}

#[test]
fn a_label_reference_holds_the_name_even_with_spaces_and_colons() {
    let s = scope();
    let r = s.label_ref("good first: issue");
    let back: Ref = r.to_string().parse().unwrap();
    assert_eq!(s.label_name(&back).unwrap(), "good first: issue");
    assert_eq!(
        s.label_name(&s.task_ref(1)).err(),
        Some(CapError::invalid("labels"))
    );
}

#[test]
fn a_list_path_carries_the_filters_and_a_search_carries_the_text() {
    let s = scope();
    let q = Query {
        status: vec![Category::Done, Category::Canceled],
        labels: vec![s.label_ref("good first")],
        assignee: Some("octo cat".into()),
        sort: Sort::Created,
        limit: Some(500),
        ..Query::default()
    };
    assert_eq!(
        s.list_path(&q).unwrap(),
        "/repos/acme/widgets/issues?state=closed&sort=created&direction=desc&per_page=100&labels=good%20first&assignee=octo%20cat"
    );
    let open = Query {
        status: vec![Category::Todo, Category::InReview],
        ..Query::default()
    };
    assert!(
        s.list_path(&open)
            .unwrap()
            .contains("state=open&sort=updated")
    );
    let mixed = Query {
        status: vec![Category::Todo, Category::Done],
        ..Query::default()
    };
    assert!(s.list_path(&mixed).unwrap().contains("state=all"));
    let text = Query {
        text: Some("crash on save".into()),
        status: vec![Category::Done],
        ..Query::default()
    };
    assert_eq!(
        s.list_path(&text).unwrap(),
        format!(
            "/search/issues?q={}&sort=updated&order=desc&per_page=50",
            encode("repo:acme/widgets is:issue crash on save is:closed")
        )
    );
    let by_priority = Query {
        sort: Sort::Priority,
        ..Query::default()
    };
    assert!(matches!(
        s.list_path(&by_priority),
        Err(CapError::Unsupported { .. })
    ));
}

#[test]
fn only_a_cursor_for_this_repository_is_followed() {
    let s = scope();
    for ok in [
        "/repos/acme/widgets/issues?page=2",
        "/repositories/12/issues?after=abc",
        "/search/issues?q=x&page=2",
    ] {
        assert_eq!(s.cursor_path(ok).unwrap(), ok);
    }
    for bad in [
        "/repos/acme/other/issues?page=2",
        "/user",
        "https://evil.example/repos/acme/widgets/",
        "/repos/acme/widgets/../../user",
        "",
    ] {
        assert_eq!(
            s.cursor_path(bad).err(),
            Some(CapError::invalid("cursor")),
            "{bad:?}"
        );
    }
}

#[test]
fn the_next_page_is_the_path_of_the_link_header() {
    let link = r#"<https://api.github.com/repositories/1/issues?per_page=3&after=Y3Vy>; rel="next", <https://api.github.com/repositories/1/issues?per_page=3&before=Zm9v>; rel="prev""#;
    assert_eq!(
        next_cursor(Some(link)).as_deref(),
        Some("/repositories/1/issues?per_page=3&after=Y3Vy")
    );
    assert_eq!(
        next_cursor(Some(r#"<https://api.github.com/x?page=1>; rel="prev""#)),
        None
    );
    assert_eq!(next_cursor(None), None);
    assert_eq!(encode("a b/c:é"), "a%20b%2Fc%3A%C3%A9");
}

#[test]
fn an_agents_comment_carries_its_actor_in_a_hidden_line_and_a_persons_does_not() {
    let agent = Actor::agent("agent-1", "Claude", "alex");
    let written = marker::with_actor("Looks good.", &agent);
    assert!(written.starts_with("Looks good.\n\n<!-- atelier-actor: "));
    assert_eq!(
        marker::split(&written),
        ("Looks good.".to_string(), Some(agent))
    );
    assert_eq!(
        marker::with_actor("Hi", &Actor::person("alex", "Alex")),
        "Hi"
    );
    assert_eq!(
        marker::split("Plain <!-- not ours -->"),
        ("Plain <!-- not ours -->".to_string(), None)
    );
    assert_eq!(marker::split("x\n<!-- atelier-actor: {broken} -->").1, None);
}

fn event(value: serde_json::Value) -> TimelineEvent {
    let mut base = json!({ "created_at": "2026-01-03T00:00:00Z", "actor": { "login": "octo" } });
    base.as_object_mut()
        .unwrap()
        .extend(value.as_object().unwrap().clone());
    serde_json::from_value(base).unwrap()
}

#[test]
fn a_timeline_event_reads_as_the_activity_it_is() {
    let s = scope();
    let kind = |v: serde_json::Value| s.activity(7, &event(v), "t0").map(|a| a.kind);
    assert_eq!(
        kind(json!({ "event": "commented", "id": 5, "body": "hi" })),
        Some(ActivityKind::Commented)
    );
    assert_eq!(
        kind(json!({ "event": "labeled", "id": 1, "label": { "name": "status:in-review" } })),
        Some(ActivityKind::StatusChanged)
    );
    assert_eq!(
        kind(json!({ "event": "unlabeled", "id": 1, "label": { "name": "bug" } })),
        Some(ActivityKind::Edited)
    );
    assert_eq!(
        kind(json!({ "event": "closed", "id": 2, "state_reason": "not_planned" })),
        Some(ActivityKind::StatusChanged)
    );
    assert_eq!(
        kind(json!({ "event": "reopened", "id": 3 })),
        Some(ActivityKind::StatusChanged)
    );
    assert_eq!(
        kind(json!({ "event": "assigned", "id": 4, "assignee": { "login": "x" } })),
        Some(ActivityKind::Assigned)
    );
    assert_eq!(
        kind(json!({ "event": "renamed", "id": 6, "rename": { "from": "a", "to": "b" } })),
        Some(ActivityKind::Edited)
    );
    assert_eq!(
        kind(json!({ "event": "referenced", "id": 8, "commit_id": "abc" })),
        Some(ActivityKind::Commit)
    );
    let pr = json!({ "event": "cross-referenced", "source": { "type": "issue", "issue": { "number": 9, "title": "Fix", "pull_request": {} } } });
    assert_eq!(kind(pr.clone()), Some(ActivityKind::PrOpened));
    let cross = s.activity(7, &event(pr), "t4").unwrap();
    assert_eq!(
        cross.reference.to_string(),
        "tasks:github:acme.widgets:7:t4",
        "an event with no id is named by its slot"
    );
    assert_eq!(cross.detail.unwrap()["number"], 9);
    let plain_issue =
        json!({ "event": "cross-referenced", "source": { "issue": { "number": 9 } } });
    assert_eq!(
        kind(plain_issue),
        None,
        "a mention by an issue is not a pull request"
    );
    for noise in ["mentioned", "subscribed", "milestoned", "locked"] {
        assert_eq!(kind(json!({ "event": noise, "id": 1 })), None, "{noise}");
    }
}

#[test]
fn a_status_label_on_a_timeline_names_the_category_it_moved_to() {
    let s = scope();
    let a = s
        .activity(
            7,
            &event(
                json!({ "event": "labeled", "id": 1, "label": { "name": "status:in-progress" } }),
            ),
            "t",
        )
        .unwrap();
    assert_eq!(
        a.detail.unwrap(),
        json!({ "label": "status:in-progress", "change": "labeled", "category": "in_progress" })
    );
}
