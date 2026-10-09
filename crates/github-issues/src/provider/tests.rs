use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use atelier_capabilities::{
    Actor, CapError, Operation,
    tasks::{
        ActivityKind, Category, Change, EventKind, NewTask, Patch, Priority, Query, TasksProvider,
        contract,
    },
};
use serde_json::json;

use super::{
    GithubIssues,
    testing::{Fake, Script, fixture, reply},
};
use crate::{
    options::{Options, StatusLabels},
    runner::{Failure, Gh, GhCli, Method},
};

fn alex() -> Actor {
    Actor::person("alex", "Alex")
}

fn on(gh: Arc<dyn Gh>) -> GithubIssues {
    GithubIssues::new(gh, "acme/widgets", Options::default()).unwrap()
}

fn quick() -> Options {
    Options {
        poll_every: Duration::from_millis(40),
        ..Options::default()
    }
}

/// Waits, in small steps, until `check` holds. A watch changes on its own clock, so a test cannot know when.
fn eventually(what: &str, check: impl Fn() -> bool) {
    let start = Instant::now();
    while !check() {
        assert!(start.elapsed() < Duration::from_secs(5), "never: {what}");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn github_issues_over_a_small_github_pass_the_tasks_contract() {
    contract::run(&|| Box::new(GithubIssues::new(Fake::new(), "acme/widgets", quick()).unwrap()));
}

#[test]
fn capabilities_list_what_works_and_projects_answers_with_none() {
    let p = on(Fake::new());
    let caps = p.capabilities();
    for op in atelier_capabilities::Capabilities::CORE {
        assert!(caps.can(op), "{op:?}");
    }
    assert!(caps.can(Operation::Statuses));
    for missing in [
        Operation::Export,
        Operation::Import,
        Operation::Delete,
        Operation::CreateMany,
        Operation::Link,
    ] {
        assert!(!caps.can(missing), "{missing:?}");
    }
    assert!(caps.features.is_empty());
    assert_eq!(p.projects().unwrap(), []);
    assert_eq!(p.statuses().unwrap().len(), 6);
    assert!(matches!(
        p.delete(&p.scope.task_ref(1), &alex()),
        Err(CapError::Unsupported { .. })
    ));
    assert_eq!((p.provider(), p.account()), ("github", "acme.widgets"));
}

#[test]
fn whoami_is_the_account_of_gh() {
    let gh = Script::new(vec![(
        "GET /user",
        Ok(reply(200, &[], &fixture("user.json"))),
    )]);
    let me = on(gh).whoami().unwrap();
    assert_eq!((me.id.as_str(), me.name.as_str()), ("someone", "Some One"));
}

#[test]
fn a_page_skips_pull_requests_and_carries_githubs_cursor() {
    let list = fixture("list_page.json");
    let link = r#"<https://api.github.com/repositories/1/issues?state=all&per_page=6&after=Y3Vy>; rel="next""#;
    let gh = Script::new(vec![
        (
            "GET /repos/acme/widgets/issues?state=all&sort=updated&direction=desc&per_page=6",
            Ok(reply(200, &[("link", link)], &list)),
        ),
        (
            "GET /repositories/1/issues?state=all&per_page=6&after=Y3Vy",
            Ok(reply(200, &[], &json!([]))),
        ),
    ]);
    let p = on(gh.clone());
    let page = p
        .list(&Query {
            limit: Some(6),
            ..Query::default()
        })
        .unwrap();
    let prs: Vec<u64> = list
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i.get("pull_request").is_some())
        .map(|i| i["number"].as_u64().unwrap())
        .collect();
    assert_eq!(
        (list.as_array().unwrap().len(), prs.len()),
        (6, 3),
        "the fixture mixes issues and pull requests"
    );
    assert_eq!(page.items.len(), 3);
    assert!(
        page.items
            .iter()
            .all(|t| !prs.contains(&t.key[1..].parse().unwrap()))
    );
    assert_eq!(
        page.next_cursor.as_deref(),
        Some("/repositories/1/issues?state=all&per_page=6&after=Y3Vy")
    );
    let last = p
        .list(&Query {
            cursor: page.next_cursor,
            ..Query::default()
        })
        .unwrap();
    assert_eq!((last.items.len(), last.next_cursor), (0, None));
}

#[test]
fn a_cursor_for_another_place_is_refused_before_any_call() {
    let gh = Script::new(vec![]);
    let q = Query {
        cursor: Some("/user".into()),
        ..Query::default()
    };
    assert_eq!(on(gh).list(&q).err(), Some(CapError::invalid("cursor")));
}

#[test]
fn filters_that_github_cannot_answer_are_unsupported_not_ignored() {
    let p = on(Script::new(vec![]));
    let project = Query {
        project: Some(p.scope.task_ref(1)),
        ..Query::default()
    };
    let linked = Query {
        linked_to: Some("x".into()),
        ..Query::default()
    };
    for q in [project, linked] {
        assert!(matches!(p.list(&q), Err(CapError::Unsupported { .. })));
    }
}

#[test]
fn a_text_query_goes_through_search_and_reads_its_items() {
    let item = fixture("issue_labelled_completed.json");
    let q = "repo%3Aacme%2Fwidgets%20is%3Aissue%20crash";
    let gh = Script::new(vec![(
        &format!("GET /search/issues?q={q}&sort=updated&order=desc&per_page=50"),
        Ok(reply(
            200,
            &[],
            &json!({ "total_count": 1, "items": [item] }),
        )),
    )]);
    let page = on(gh)
        .list(&Query {
            text: Some("crash".into()),
            ..Query::default()
        })
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].key, "#74");
}

#[test]
fn a_real_issue_reads_as_a_done_task_with_its_labels_and_its_raw_json() {
    let raw = fixture("issue_labelled_completed.json");
    let gh = Script::new(vec![(
        "GET /repos/acme/widgets/issues/74",
        Ok(reply(200, &[], &raw)),
    )]);
    let p = on(gh);
    let t = p.get(&p.scope.task_ref(74)).unwrap();
    assert_eq!(t.status.category, Category::Done);
    assert_eq!(t.reference.to_string(), "tasks:github:acme.widgets:74");
    let names: Vec<_> = t
        .labels
        .iter()
        .map(|l| p.scope.label_name(l).unwrap())
        .collect();
    assert_eq!(names, ["enhancement", "core"]);
    assert_eq!(t.version, raw["updated_at"].as_str().unwrap());
    assert_eq!(t.raw.as_ref().unwrap(), &raw);
    assert!(t.links.is_empty() && t.project.is_none() && t.parent.is_none());
}

#[test]
fn a_not_planned_close_is_canceled() {
    let raw = fixture("issue_not_planned.json");
    let gh = Script::new(vec![(
        "GET /repos/acme/widgets/issues/14626",
        Ok(reply(200, &[], &raw)),
    )]);
    let p = on(gh);
    assert_eq!(
        p.get(&p.scope.task_ref(14626)).unwrap().status.category,
        Category::Canceled
    );
}

#[test]
fn a_pull_request_is_not_a_task() {
    let list = fixture("list_page.json");
    let pr = list
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i.get("pull_request").is_some())
        .unwrap()
        .clone();
    let n = pr["number"].as_u64().unwrap();
    let gh = Script::new(vec![(
        &format!("GET /repos/acme/widgets/issues/{n}"),
        Ok(reply(200, &[], &pr)),
    )]);
    let p = on(gh);
    assert!(matches!(
        p.get(&p.scope.task_ref(n)),
        Err(CapError::NotFound { .. })
    ));
}

#[test]
fn a_stale_version_conflicts_after_one_read_and_writes_nothing() {
    let raw = fixture("issue_labelled_completed.json");
    let gh = Script::new(vec![(
        "GET /repos/acme/widgets/issues/74",
        Ok(reply(200, &[], &raw)),
    )]);
    let p = on(gh.clone());
    let patch = Patch {
        title: Some("New".into()),
        ..Patch::default()
    };
    match p.update(
        &p.scope.task_ref(74),
        &patch,
        "2001-01-01T00:00:00Z",
        &alex(),
    ) {
        Err(CapError::Conflict { current }) => assert_eq!(current["title"], raw["title"]),
        other => panic!("{other:?}"),
    }
    assert_eq!(gh.seen().len(), 1, "no PATCH was sent");
}

#[test]
fn an_update_sends_only_what_changed() {
    let raw = fixture("issue_labelled_completed.json");
    let version = raw["updated_at"].as_str().unwrap().to_string();
    let mut after = raw.clone();
    after["title"] = json!("New");
    let gh = Script::new(vec![
        (
            "GET /repos/acme/widgets/issues/74",
            Ok(reply(200, &[], &raw)),
        ),
        (
            "PATCH /repos/acme/widgets/issues/74",
            Ok(reply(200, &[], &after)),
        ),
    ]);
    let p = on(gh.clone());
    let patch = Patch {
        title: Some("New".into()),
        description: Change::Set(raw["body"].as_str().unwrap().into()),
        priority: Some(Priority::None),
        ..Patch::default()
    };
    assert_eq!(
        p.update(&p.scope.task_ref(74), &patch, &version, &alex())
            .unwrap()
            .title,
        "New"
    );
    let sent = gh.seen()[1].clone();
    assert_eq!(sent.method, Method::Patch);
    assert_eq!(
        sent.body.unwrap(),
        json!({ "title": "New" }),
        "the same body and no priority label are not sent"
    );
}

#[test]
fn a_status_moves_the_state_and_its_label_and_a_labels_patch_keeps_the_status_and_priority_labels()
{
    let p = on(Fake::new());
    let t = p
        .create(
            &NewTask {
                priority: Priority::High,
                labels: vec![p.scope.label_ref("bug")],
                ..NewTask::titled("Move")
            },
            &alex(),
        )
        .unwrap();
    let names = |t: &atelier_capabilities::tasks::Task| -> Vec<String> {
        let mut v: Vec<String> = t.raw.as_ref().unwrap()["labels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["name"].as_str().unwrap().to_string())
            .collect();
        v.sort();
        v
    };
    assert_eq!(names(&t), ["bug", "priority:high"]);
    let status = |id: &str| Patch {
        status: Some(id.into()),
        ..Patch::default()
    };
    let t = p
        .update(&t.reference, &status("in_progress"), &t.version, &alex())
        .unwrap();
    assert_eq!(
        (t.status.category, names(&t)),
        (
            Category::InProgress,
            vec![
                "bug".to_string(),
                "priority:high".into(),
                "status:in-progress".into()
            ]
        )
    );
    let relabel = Patch {
        labels: Some(vec![p.scope.label_ref("docs")]),
        ..Patch::default()
    };
    let t = p
        .update(&t.reference, &relabel, &t.version, &alex())
        .unwrap();
    assert_eq!(
        names(&t),
        ["docs", "priority:high", "status:in-progress"],
        "bug went, the control labels stayed"
    );
    assert_eq!(
        (t.priority, t.status.category),
        (Priority::High, Category::InProgress)
    );
    let t = p
        .update(&t.reference, &status("in_review"), &t.version, &alex())
        .unwrap();
    assert_eq!(names(&t), ["docs", "priority:high", "status:in-review"]);
    let t = p
        .update(&t.reference, &status("canceled"), &t.version, &alex())
        .unwrap();
    assert_eq!(t.status.category, Category::Canceled);
    assert_eq!(t.raw.as_ref().unwrap()["state_reason"], "not_planned");
    assert_eq!(
        names(&t),
        ["docs", "priority:high"],
        "a closed issue carries no status label"
    );
    let t = p
        .update(&t.reference, &status("done"), &t.version, &alex())
        .unwrap();
    assert_eq!(
        (
            t.status.category,
            t.raw.as_ref().unwrap()["state_reason"].as_str()
        ),
        (Category::Done, Some("completed"))
    );
    let t = p
        .update(&t.reference, &status("todo"), &t.version, &alex())
        .unwrap();
    assert_eq!(
        (t.status.category, t.raw.as_ref().unwrap()["state"].as_str()),
        (Category::Todo, Some("open"))
    );
    let t = p
        .update(
            &t.reference,
            &Patch {
                priority: Some(Priority::None),
                ..Patch::default()
            },
            &t.version,
            &alex(),
        )
        .unwrap();
    assert_eq!(
        (t.priority, names(&t)),
        (Priority::None, vec!["docs".to_string()])
    );
    assert_eq!(
        p.update(&t.reference, &status("nope"), &t.version, &alex())
            .err(),
        Some(CapError::invalid("status"))
    );
}

#[test]
fn a_task_can_be_born_in_any_status_and_with_custom_label_names() {
    let options = Options {
        status_labels: StatusLabels {
            in_review: "needs-review".into(),
            ..StatusLabels::default()
        },
        ..Options::default()
    };
    let p = GithubIssues::new(Fake::new(), "acme/widgets", options).unwrap();
    let born = |id: &str| {
        p.create(
            &NewTask {
                status: Some(id.into()),
                ..NewTask::titled("Born")
            },
            &alex(),
        )
        .unwrap()
    };
    let review = born("in_review");
    assert_eq!(review.status.category, Category::InReview);
    assert_eq!(review.raw.unwrap()["labels"][0]["name"], "needs-review");
    assert_eq!(born("backlog").status.category, Category::Backlog);
    assert_eq!(born("done").status.category, Category::Done);
    assert_eq!(born("canceled").status.category, Category::Canceled);
    assert_eq!(born("todo").status.category, Category::Todo);
}

#[test]
fn what_github_has_no_field_for_is_refused() {
    let p = on(Fake::new());
    let project = NewTask {
        project: Some(p.scope.task_ref(1)),
        ..NewTask::titled("x")
    };
    assert!(matches!(
        p.create(&project, &alex()),
        Err(CapError::Unsupported { .. })
    ));
    let child = NewTask {
        parent: Some(p.scope.task_ref(1)),
        ..NewTask::titled("x")
    };
    assert!(matches!(
        p.create(&child, &alex()),
        Err(CapError::Unsupported { .. })
    ));
    let t = p.create(&NewTask::titled("x"), &alex()).unwrap();
    let set = Patch {
        project: Change::Set(p.scope.task_ref(1)),
        ..Patch::default()
    };
    assert!(matches!(
        p.update(&t.reference, &set, &t.version, &alex()),
        Err(CapError::Unsupported { .. })
    ));
    let clear = Patch {
        project: Change::Clear,
        parent: Change::Clear,
        ..Patch::default()
    };
    assert_eq!(
        p.update(&t.reference, &clear, &t.version, &alex()).unwrap(),
        t,
        "clearing what is not there changes nothing"
    );
}

#[test]
fn assignees_are_logins_and_an_empty_list_clears() {
    let p = on(Fake::new());
    let t = p
        .create(
            &NewTask {
                assignees: vec!["octo".into()],
                ..NewTask::titled("A")
            },
            &alex(),
        )
        .unwrap();
    assert_eq!(t.assignees[0].id, "octo");
    let t = p
        .update(
            &t.reference,
            &Patch {
                assignees: Some(vec![]),
                ..Patch::default()
            },
            &t.version,
            &alex(),
        )
        .unwrap();
    assert!(t.assignees.is_empty());
}

#[test]
fn an_agents_comment_comes_back_as_the_agent_and_in_the_activity() {
    let p = on(Fake::new());
    let t = p.create(&NewTask::titled("Talk"), &alex()).unwrap();
    let agent = Actor::agent("agent-1", "Claude", "alex");
    p.comment(&t.reference, "Person line", &alex()).unwrap();
    let c = p.comment(&t.reference, "Agent line", &agent).unwrap();
    assert_eq!(
        (c.body.as_str(), c.author.clone()),
        ("Agent line", agent.clone())
    );
    let page = p.activity(&t.reference, None).unwrap();
    let kinds: Vec<_> = page.items.iter().map(|a| a.kind).collect();
    assert_eq!(
        kinds,
        [
            ActivityKind::Created,
            ActivityKind::Commented,
            ActivityKind::Commented
        ]
    );
    assert_eq!(
        page.items[1].by.id, "tester",
        "a person's comment is the account of gh"
    );
    assert_eq!(page.items[2].by, agent);
    assert_eq!(page.items[2].detail.as_ref().unwrap()["body"], "Agent line");
    assert!(matches!(
        p.comment(&t.reference, "  ", &alex()),
        Err(CapError::Invalid { .. })
    ));
}

#[test]
fn the_activity_of_a_real_timeline_starts_with_the_creation_and_follows_its_cursor() {
    let issue = fixture("issue_labelled_completed.json");
    let timeline = fixture("timeline.json");
    let link = r#"<https://api.github.com/repositories/1/issues/74/timeline?per_page=50&page=2>; rel="next""#;
    let gh = Script::new(vec![
        (
            "GET /repos/acme/widgets/issues/74",
            Ok(reply(200, &[], &issue)),
        ),
        (
            "GET /repos/acme/widgets/issues/74/timeline?per_page=50",
            Ok(reply(200, &[("link", link)], &timeline)),
        ),
        (
            "GET /repositories/1/issues/74/timeline?per_page=50&page=2",
            Ok(reply(200, &[], &json!([]))),
        ),
    ]);
    let p = on(gh);
    let first = p.activity(&p.scope.task_ref(74), None).unwrap();
    assert_eq!(first.items[0].kind, ActivityKind::Created);
    let kinds: Vec<_> = first.items.iter().map(|a| a.kind).collect();
    assert!(
        kinds.contains(&ActivityKind::Commented)
            && (kinds.contains(&ActivityKind::StatusChanged)
                || kinds.contains(&ActivityKind::Edited)),
        "{kinds:?}"
    );
    assert!(
        first.items.windows(2).all(|w| w[0].at <= w[1].at),
        "oldest first"
    );
    let rest = p
        .activity(&p.scope.task_ref(74), first.next_cursor.as_deref())
        .unwrap();
    assert_eq!(
        (rest.items.len(), rest.next_cursor),
        (0, None),
        "the cursor page has no second creation"
    );
}

#[test]
fn labels_list_the_repositorys_labels_without_the_control_ones() {
    let mut labels = fixture("labels.json");
    labels
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": 8, "name": "status:in-progress", "color": "fff" }));
    let gh = Script::new(vec![(
        "GET /repos/acme/widgets/labels?per_page=100",
        Ok(reply(200, &[], &labels)),
    )]);
    let p = on(gh);
    let got = p.labels().unwrap();
    assert_eq!(got.len(), labels.as_array().unwrap().len() - 1);
    assert!(got.iter().all(|l| l.name != "status:in-progress"));
    assert_eq!(
        got[0].reference.to_string(),
        format!("tasks:github:acme.widgets:label:{}", got[0].name)
    );
}

#[test]
fn errors_are_the_ones_the_screen_knows() {
    let fake = Fake::new();
    let p = on(fake.clone());
    let ask = |answer| {
        fake.answer_next(answer);
        p.whoami().err().unwrap()
    };
    assert_eq!(ask(Err(Failure::NotSignedIn)), CapError::NotSignedIn);
    assert_eq!(ask(Err(Failure::Offline)), CapError::Offline);
    assert!(
        matches!(ask(Err(Failure::ToolMissing)), CapError::Provider { code, .. } if code == "gh_missing")
    );
    assert_eq!(
        ask(Ok(reply(
            401,
            &[],
            &json!({ "message": "Bad credentials" })
        ))),
        CapError::NotSignedIn
    );
    assert_eq!(
        ask(Ok(reply(
            403,
            &[("retry-after", "7")],
            &json!({ "message": "secondary rate limit" })
        ))),
        CapError::RateLimited {
            retry_after_ms: 7000
        }
    );
    assert!(matches!(
        ask(Ok(reply(500, &[], &json!({})))),
        CapError::Provider { .. }
    ));
    assert!(matches!(
        p.get(&p.scope.task_ref(999)),
        Err(CapError::NotFound { .. })
    ));
    assert!(matches!(
        p.create(&NewTask::titled("  "), &alex()),
        Err(CapError::Invalid { .. })
    ));
}

#[test]
fn a_subscription_tells_what_others_do_and_asks_with_the_etag() {
    let fake = Fake::new();
    let existing = fake.external_issue("Before");
    let p = GithubIssues::new(fake.clone(), "acme/widgets", quick()).unwrap();
    let sub = p.subscribe().unwrap();
    let pr = fake.external_pull_request("A pull request");
    let made = fake.external_issue("Opened by someone");
    fake.external_title(existing, "Renamed by someone");
    let mut told = Vec::new();
    while told.len() < 2 {
        let event = sub.recv_timeout(Duration::from_secs(5)).expect("an event");
        told.push((event.kind, event.task.key, event.task.title));
    }
    assert_eq!(
        told[0],
        (
            EventKind::Created,
            format!("#{made}"),
            "Opened by someone".into()
        )
    );
    assert_eq!(
        told[1],
        (
            EventKind::Updated,
            format!("#{existing}"),
            "Renamed by someone".into()
        )
    );
    assert!(
        told.iter().all(|t| t.1 != format!("#{pr}")),
        "a pull request is not a task"
    );
    eventually("a poll that nothing changed for", || {
        fake.calls()
            .iter()
            .filter(|c| c.ends_with("[etag]"))
            .count()
            >= 2
    });
    assert!(
        sub.try_recv().is_err(),
        "a poll that found nothing new tells nothing"
    );
}

#[test]
fn the_watch_ends_when_the_last_subscription_drops() {
    let fake = Fake::new();
    let p = GithubIssues::new(fake.clone(), "acme/widgets", quick()).unwrap();
    let (first, second) = (p.subscribe().unwrap(), p.subscribe().unwrap());
    drop(first);
    thread::sleep(Duration::from_millis(150));
    assert!(p.hub.lock().unwrap().running, "one reader is still there");
    let t = p.create(&NewTask::titled("Heard"), &alex()).unwrap();
    assert_eq!(
        second
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .task
            .reference,
        t.reference
    );
    drop(second);
    eventually("the watch to end", || !p.hub.lock().unwrap().running);
    let calls = fake.calls().len();
    thread::sleep(Duration::from_millis(200));
    assert_eq!(fake.calls().len(), calls, "no poll after the watch ended");
    // A new subscription starts a new watch.
    let again = p.subscribe().unwrap();
    assert!(p.hub.lock().unwrap().running);
    drop(again);
}

#[test]
fn subscribing_while_signed_out_fails_for_the_caller() {
    let fake = Fake::new();
    fake.answer_next(Err(Failure::NotSignedIn));
    let p = on(fake);
    assert_eq!(p.subscribe().err(), Some(CapError::NotSignedIn));
    assert!(!p.hub.lock().unwrap().running);
}

/// Reads a public repository with the `gh` of this host. It only reads: it creates, edits and comments on nothing.
/// Run it with `cargo test -p atelier-github-issues -- --ignored`.
#[test]
#[ignore = "calls the real GitHub through the signed-in gh"]
fn live_smoke_reads_issues_of_a_public_repository() {
    let p = GithubIssues::new(Arc::new(GhCli::default()), "cli/cli", Options::default()).unwrap();
    assert!(!p.whoami().unwrap().id.is_empty());
    let page = p
        .list(&Query {
            limit: Some(5),
            ..Query::default()
        })
        .unwrap();
    assert!(!page.items.is_empty() && page.items.len() <= 5);
    assert!(page.next_cursor.is_some());
    let first = &page.items[0];
    let again = p.get(&first.reference).unwrap();
    assert_eq!(
        (again.key.as_str(), again.title.as_str()),
        (first.key.as_str(), first.title.as_str())
    );
    assert!(p.labels().unwrap().len() > 5);
    let timeline = p.activity(&first.reference, None).unwrap();
    assert_eq!(timeline.items[0].kind, ActivityKind::Created);
}
