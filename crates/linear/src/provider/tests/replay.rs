//! The provider against a local server that replays the fixtures: the real HTTP client, the paging, the error
//! mapping, the conflict check and every write path. No call here leaves the machine.
use std::{thread, time::Duration};

use atelier_capabilities::{
    Actor, CapError, Operation, Ref,
    tasks::{
        Category, Change, EventKind, NewTask, Patch, Priority, Query, TasksProvider, contract,
    },
};
use serde_json::{Value, json};

use super::{
    fixture,
    server::{Call, Reply, Server},
};
use crate::{Config, LinearTasks};

const KEY: &str = "lin_api_secret_value_1234567890";

/// The replies every test shares: who the key is, the teams, and one issue by key.
fn standard(call: &Call) -> Reply {
    match call.operation.as_str() {
        "Viewer" => Reply::json(&fixture("viewer")),
        "Teams" => Reply::json(&fixture("teams")),
        "Get" => Reply::json(&fixture("get")),
        "List" if call.variables["after"].is_null() => Reply::json(&fixture("list")),
        "List" => Reply::json(&fixture("list_page2")),
        "Labels" => Reply::json(&fixture("labels")),
        "Projects" => Reply::json(&fixture("projects")),
        "States" => Reply::json(&fixture("states")),
        "Comments" => Reply::json(&fixture("comments")),
        "History" => Reply::json(&fixture("history")),
        other => panic!("the test server was asked for {other}"),
    }
}

fn connect(server: &Server) -> LinearTasks {
    LinearTasks::connect_with(Config::new(KEY).with_endpoint(&server.url)).expect("connect")
}

fn alex() -> Actor {
    Actor::person("u1", "Alex")
}

fn issue_ref() -> Ref {
    let key = fixture("get")["data"]["issue"]["identifier"]
        .as_str()
        .unwrap()
        .to_string();
    format!("tasks:linear:acme:{key}").parse().unwrap()
}

/// The `get` fixture with another `updatedAt`, as Linear shows an issue after somebody changed it.
fn changed(updated_at: &str, title: &str) -> Value {
    let mut v = fixture("get");
    v["data"]["issue"]["updatedAt"] = json!(updated_at);
    v["data"]["issue"]["title"] = json!(title);
    v
}

#[test]
fn connect_reads_the_workspace_and_the_key_goes_in_the_header_bare() {
    let server = Server::start(standard);
    let linear = connect(&server);
    assert_eq!((linear.provider(), linear.account()), ("linear", "acme"));
    assert_eq!(server.calls()[0].authorization, KEY);
    let me = linear.whoami().unwrap();
    assert_eq!(me.name, "Person 1");
}

#[test]
fn an_oauth_token_is_sent_as_a_bearer() {
    let server = Server::start(standard);
    LinearTasks::connect_with(Config::new("lin_oauth_abc").with_endpoint(&server.url)).unwrap();
    assert_eq!(server.calls()[0].authorization, "Bearer lin_oauth_abc");
}

#[test]
fn a_list_pages_by_linears_cursor() {
    let server = Server::start(standard);
    let linear = connect(&server);
    let first = linear
        .list(&Query {
            limit: Some(3),
            ..Query::default()
        })
        .unwrap();
    assert_eq!(first.items.len(), 3);
    let cursor = first.next_cursor.clone().expect("more pages");
    let second = linear
        .list(&Query {
            limit: Some(3),
            cursor: Some(cursor.clone()),
            ..Query::default()
        })
        .unwrap();
    let lists = server.calls_of("List");
    assert_eq!(lists[0].variables["first"], 3);
    assert_eq!(lists[0].variables["after"], Value::Null);
    assert_eq!(
        lists[1].variables["after"], cursor,
        "the second call carries the cursor"
    );
    assert_eq!(lists[1].variables["orderBy"], "updatedAt");
    let keys: Vec<_> = first
        .items
        .iter()
        .chain(&second.items)
        .map(|t| t.key.clone())
        .collect();
    let mut unique = keys.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(keys.len(), unique.len(), "no task twice across the pages");
}

#[test]
fn a_list_sends_its_filter_and_clamps_its_page_size() {
    let server = Server::start(standard);
    let linear =
        LinearTasks::connect_with(Config::new(KEY).with_endpoint(&server.url).with_team("ACM"))
            .unwrap();
    linear
        .list(&Query {
            status: vec![Category::InReview],
            limit: Some(10_000),
            ..Query::default()
        })
        .unwrap();
    let list = &server.calls_of("List")[0];
    assert_eq!(list.variables["first"], 250, "Linear's own ceiling");
    assert_eq!(
        list.variables["filter"],
        json!({ "and": [
            { "team": { "key": { "eq": "ACM" } } },
            { "or": [ { "state": { "type": { "eq": "started" }, "name": { "containsIgnoreCase": "review" } } } ] },
        ] })
    );
}

#[test]
fn get_reads_one_task_and_a_missing_one_is_not_found() {
    let server = Server::start(|call| match call.operation.as_str() {
        "Get" if call.variables["id"] == "ACM-404" => Reply::json(&fixture("get_missing")),
        _ => standard(call),
    });
    let linear = connect(&server);
    let task = linear.get(&issue_ref()).unwrap();
    assert_eq!(task.reference, issue_ref());
    let missing: Ref = "tasks:linear:acme:ACM-404".parse().unwrap();
    assert!(matches!(
        linear.get(&missing),
        Err(CapError::NotFound { .. })
    ));
    let elsewhere: Ref = "tasks:linear:other:ACM-1".parse().unwrap();
    assert!(matches!(
        linear.get(&elsewhere),
        Err(CapError::NotFound { .. })
    ));
    assert_eq!(
        server.calls_of("Get").len(),
        2,
        "another workspace's reference never reaches Linear"
    );
}

#[test]
fn http_401_is_not_signed_in() {
    let server = Server::start(|call| {
        if call.operation == "Viewer" {
            Reply::json(&fixture("viewer"))
        } else {
            Reply::status(401)
        }
    });
    let linear = connect(&server);
    assert_eq!(linear.get(&issue_ref()).unwrap_err(), CapError::NotSignedIn);
    let at_connect = LinearTasks::connect_with(
        Config::new(KEY).with_endpoint(Server::start(|_| Reply::status(401)).url.as_str()),
    );
    assert_eq!(at_connect.unwrap_err(), CapError::NotSignedIn);
}

#[test]
fn http_429_is_rate_limited_with_the_retry_after() {
    let server = Server::start(|call| {
        if call.operation == "Viewer" {
            Reply::json(&fixture("viewer"))
        } else {
            Reply::status(429).with_header("Retry-After", "7")
        }
    });
    let linear = connect(&server);
    assert_eq!(
        linear.list(&Query::default()).unwrap_err(),
        CapError::RateLimited {
            retry_after_ms: 7000
        }
    );
}

#[test]
fn a_rate_limit_without_retry_after_still_says_to_wait() {
    let server = Server::start(|call| {
        if call.operation == "Viewer" {
            Reply::json(&fixture("viewer"))
        } else {
            Reply::status(429)
        }
    });
    match connect(&server).list(&Query::default()) {
        Err(CapError::RateLimited { retry_after_ms }) => assert!(retry_after_ms >= 1000),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_ratelimited_graphql_error_is_rate_limited_too() {
    let limited =
        json!({ "errors": [{ "message": "slow down", "extensions": { "code": "RATELIMITED" } }] });
    let server = Server::start(move |call| {
        if call.operation == "Viewer" {
            Reply::json(&fixture("viewer"))
        } else {
            Reply::json(&limited).with_header("Retry-After", "2")
        }
    });
    assert_eq!(
        connect(&server).labels().unwrap_err(),
        CapError::RateLimited {
            retry_after_ms: 2000
        }
    );
}

#[test]
fn no_connection_is_offline() {
    let result =
        LinearTasks::connect_with(Config::new(KEY).with_endpoint(Server::closed_address()));
    assert_eq!(result.unwrap_err(), CapError::Offline);
}

#[test]
fn a_graphql_error_is_a_provider_error_with_linears_code_and_words() {
    let failed = json!({ "errors": [{ "message": "Argument Validation Error", "extensions": { "code": "INVALID_INPUT" } }], "data": null });
    let server = Server::start(move |call| {
        if call.operation == "Viewer" {
            Reply::json(&fixture("viewer"))
        } else {
            Reply::json(&failed)
        }
    });
    assert_eq!(
        connect(&server).projects().unwrap_err(),
        CapError::Provider {
            code: "INVALID_INPUT".into(),
            message: "Argument Validation Error".into()
        }
    );
}

#[test]
fn a_page_that_is_not_json_is_a_provider_error() {
    let server = Server::start(|call| {
        if call.operation == "Viewer" {
            Reply::json(&fixture("viewer"))
        } else {
            Reply {
                status: 502,
                headers: vec![],
                body: "<html>bad gateway</html>".into(),
            }
        }
    });
    assert!(
        matches!(connect(&server).labels(), Err(CapError::Provider { code, .. }) if code == "http_502")
    );
}

#[test]
fn the_key_is_in_no_debug_text_and_no_error() {
    let server = Server::start(|_| Reply::status(401));
    let config = Config::new(KEY).with_endpoint(&server.url);
    assert!(!format!("{config:?}").contains(KEY));
    let error = LinearTasks::connect_with(config).unwrap_err();
    assert!(!format!("{error:?} {error}").contains(KEY));
    let ok = Server::start(standard);
    let linear = connect(&ok);
    assert!(!format!("{linear:?}").contains(KEY));
    let offline =
        LinearTasks::connect_with(Config::new(KEY).with_endpoint(Server::closed_address()))
            .unwrap_err();
    assert!(!format!("{offline:?}").contains(KEY));
}

#[test]
fn an_update_with_an_old_version_is_a_conflict_and_writes_nothing() {
    let server = Server::start(|call| match call.operation.as_str() {
        "Get" => Reply::json(&changed(
            "2026-10-09T05:00:00.000Z",
            "Changed by someone else",
        )),
        _ => standard(call),
    });
    let linear = connect(&server);
    let patch = Patch {
        title: Some("Mine".into()),
        ..Patch::default()
    };
    match linear.update(&issue_ref(), &patch, "2026-10-09T01:00:00.000Z", &alex()) {
        Err(CapError::Conflict { current }) => {
            assert_eq!(
                current["title"], "Changed by someone else",
                "the conflict carries the task as it is"
            );
            assert_eq!(current["version"], "2026-10-09T05:00:00.000Z");
        }
        other => panic!("{other:?}"),
    }
    assert!(
        server.calls_of("Update").is_empty(),
        "the stale write never reached Linear"
    );
}

#[test]
fn an_update_with_the_current_version_sends_only_the_patch() {
    let current = fixture("get")["data"]["issue"]["updatedAt"]
        .as_str()
        .unwrap()
        .to_string();
    let server = Server::start(|call| match call.operation.as_str() {
        "Update" => Reply::json(&json!({ "data": { "issueUpdate": { "success": true,
            "issue": changed("2026-10-09T06:00:00.000Z", "New title")["data"]["issue"] } } })),
        _ => standard(call),
    });
    let linear = connect(&server);
    let patch = Patch {
        title: Some("New title".into()),
        priority: Some(Priority::Low),
        description: Change::Clear,
        ..Patch::default()
    };
    let after = linear
        .update(&issue_ref(), &patch, &current, &alex())
        .unwrap();
    assert_eq!(after.title, "New title");
    assert_ne!(after.version, current, "the version moved");
    let update = &server.calls_of("Update")[0];
    assert_eq!(update.variables["id"], issue_ref().id);
    assert_eq!(
        update.variables["input"],
        json!({ "title": "New title", "priority": 4, "description": "" })
    );
}

#[test]
fn an_empty_patch_changes_nothing_and_keeps_the_version() {
    let current = fixture("get")["data"]["issue"]["updatedAt"]
        .as_str()
        .unwrap()
        .to_string();
    let server = Server::start(standard);
    let linear = connect(&server);
    let same = linear
        .update(&issue_ref(), &Patch::default(), &current, &alex())
        .unwrap();
    assert_eq!(same.version, current);
    assert!(server.calls_of("Update").is_empty());
}

#[test]
fn a_new_parent_is_read_to_its_uuid_before_the_update() {
    let current = fixture("get")["data"]["issue"]["updatedAt"]
        .as_str()
        .unwrap()
        .to_string();
    let server = Server::start(|call| match call.operation.as_str() {
        "Update" => Reply::json(
            &json!({ "data": { "issueUpdate": { "success": true, "issue": fixture("get")["data"]["issue"] } } }),
        ),
        _ => standard(call),
    });
    let linear = connect(&server);
    let parent: Ref = "tasks:linear:acme:ACM-9".parse().unwrap();
    linear
        .update(
            &issue_ref(),
            &Patch {
                parent: Change::Set(parent),
                ..Patch::default()
            },
            &current,
            &alex(),
        )
        .unwrap();
    let uuid = fixture("get")["data"]["issue"]["id"].clone();
    assert_eq!(
        server.calls_of("Update")[0].variables["input"],
        json!({ "parentId": uuid })
    );
}

#[test]
fn create_sends_the_team_and_what_was_asked_and_returns_the_task() {
    let server = Server::start(|call| match call.operation.as_str() {
        "Create" => Reply::json(
            &json!({ "data": { "issueCreate": { "success": true, "issue": fixture("get")["data"]["issue"] } } }),
        ),
        _ => standard(call),
    });
    let linear = connect(&server);
    let made = linear
        .create(
            &NewTask {
                description: "Notes".into(),
                priority: Priority::High,
                ..NewTask::titled(" Fix it ")
            },
            &alex(),
        )
        .unwrap();
    assert_eq!(made.reference, issue_ref());
    let team = fixture("teams")["data"]["teams"]["nodes"][0]["id"].clone();
    assert_eq!(
        server.calls_of("Create")[0].variables["input"],
        json!({ "teamId": team, "title": "Fix it", "description": "Notes", "priority": 2 })
    );
}

#[test]
fn create_refuses_a_blank_title_and_does_not_guess_a_team() {
    let two_teams = json!({ "data": { "teams": { "nodes": [
        { "id": "t1", "key": "AAA", "name": "A" }, { "id": "t2", "key": "BBB", "name": "B" } ] } } });
    let server = Server::start(move |call| match call.operation.as_str() {
        "Teams" => Reply::json(&two_teams),
        _ => standard(call),
    });
    let linear = connect(&server);
    assert!(
        matches!(linear.create(&NewTask::titled("  "), &alex()), Err(CapError::Invalid { field }) if field == "title")
    );
    assert!(
        matches!(linear.create(&NewTask::titled("Real"), &alex()), Err(CapError::Invalid { field }) if field == "team")
    );
    assert!(server.calls_of("Create").is_empty());
}

#[test]
fn a_comment_goes_to_the_issues_uuid_and_an_agent_is_named_in_it() {
    let server = Server::start(|call| match call.operation.as_str() {
        "Comment" => {
            let mut node = fixture("comments")["data"]["issue"]["comments"]["nodes"][0].clone();
            node["body"] = call.variables["input"]["body"].clone();
            Reply::json(
                &json!({ "data": { "commentCreate": { "success": true, "comment": node } } }),
            )
        }
        _ => standard(call),
    });
    let linear = connect(&server);
    let agent = Actor::agent("a1", "Claude", "u1");
    let comment = linear.comment(&issue_ref(), "Checked.", &agent).unwrap();
    let sent = &server.calls_of("Comment")[0].variables["input"];
    assert_eq!(sent["issueId"], fixture("get")["data"]["issue"]["id"]);
    assert_eq!(sent["body"], "Checked.\n\n_Written by the agent Claude._");
    assert_eq!(comment.task, issue_ref());
    assert!(matches!(
        linear.comment(&issue_ref(), "  ", &agent),
        Err(CapError::Invalid { .. })
    ));
}

#[test]
fn activity_merges_history_and_comments() {
    // The fixtures say there is more; the second ask of each is empty and ends the paging.
    let server = Server::start(|call| match call.operation.as_str() {
        "History" | "Comments" if !call.variables["after"].is_null() => {
            let field = if call.operation == "History" {
                "history"
            } else {
                "comments"
            };
            Reply::json(
                &json!({ "data": { "issue": { field: { "nodes": [], "pageInfo": { "hasNextPage": false, "endCursor": null } } } } }),
            )
        }
        _ => standard(call),
    });
    let linear = connect(&server);
    let page = linear.activity(&issue_ref(), None).unwrap();
    assert_eq!(page.items.len(), 1 + 5 + 5);
    assert!(page.items.windows(2).all(|w| w[0].at <= w[1].at));
}

#[test]
fn labels_projects_and_statuses_read_every_page() {
    let server = Server::start(|call| match call.operation.as_str() {
        // The fixture says "more", so the second call must follow the cursor and then stop.
        "Labels" if call.variables["after"].is_null() => Reply::json(&fixture("labels")),
        "Labels" => {
            let mut last = fixture("labels");
            last["data"]["issueLabels"]["pageInfo"] =
                json!({ "hasNextPage": false, "endCursor": null });
            Reply::json(&last)
        }
        _ => standard(call),
    });
    let linear = connect(&server);
    assert_eq!(linear.labels().unwrap().len(), 10);
    assert_eq!(
        server.calls_of("Labels")[1].variables["after"],
        "00000000-0000-4000-8000-000000000022"
    );
    assert_eq!(server.calls_of("Labels")[0].variables["first"], 250);
}

#[test]
fn capabilities_list_exactly_what_works() {
    let server = Server::start(standard);
    let linear = connect(&server);
    let caps = linear.capabilities();
    for op in [
        Operation::List,
        Operation::Get,
        Operation::Create,
        Operation::Update,
        Operation::Comment,
        Operation::Activity,
        Operation::Subscribe,
        Operation::Labels,
        Operation::Projects,
        Operation::Statuses,
    ] {
        assert!(caps.can(op), "{op:?} is listed");
    }
    for op in [
        Operation::Delete,
        Operation::Export,
        Operation::Import,
        Operation::CreateMany,
        Operation::Link,
    ] {
        assert!(!caps.can(op), "{op:?} is not listed");
    }
    let unsupported = |r: Result<(), CapError>| matches!(r, Err(CapError::Unsupported { .. }));
    assert!(unsupported(linear.delete(&issue_ref(), &alex())));
    assert!(unsupported(linear.export(None).map(|_| ())));
    assert!(unsupported(linear.import(&[]).map(|_| ())));
    assert_eq!(caps.limits.page_max, Some(250));
    assert!(
        linear
            .statuses()
            .unwrap()
            .iter()
            .any(|s| s.category == Category::InReview)
    );
}

/// Two checks of the shared contract need no state in the service, so they run against the replay server: the
/// capabilities list is honest, and a reference parses back. The others (get after create returns what was sent,
/// patches, paging, export and import, subscribe) need a service that remembers what was written. The replay server
/// does not, so they are not run: a pass there would prove the fixture, not the provider.
#[test]
fn the_contract_checks_that_need_no_memory_pass() {
    let server = Server::start(|call| match call.operation.as_str() {
        "Create" => Reply::json(
            &json!({ "data": { "issueCreate": { "success": true, "issue": fixture("get")["data"]["issue"] } } }),
        ),
        _ => standard(call),
    });
    let make = || -> Box<dyn TasksProvider> { Box::new(connect(&server)) };
    contract::capabilities_are_honest(&make);
    contract::a_reference_parses_back(&make);
}

fn recent_issue(updated_at: &str, created_at: &str) -> Value {
    let mut issue = fixture("get")["data"]["issue"].clone();
    issue["updatedAt"] = json!(updated_at);
    issue["createdAt"] = json!(created_at);
    issue
}

#[test]
fn subscribe_tells_what_changed_since_it_started_and_stops_when_dropped() {
    let server = Server::start(|call| match call.operation.as_str() {
        "Latest" => Reply::json(
            &json!({ "data": { "issues": { "nodes": [{ "updatedAt": "2026-10-09T03:00:00.000Z" }] } } }),
        ),
        "List" => {
            let since = call.variables["filter"]["and"][0]["updatedAt"]["gt"]
                .as_str()
                .unwrap()
                .to_string();
            let nodes = if since == "2026-10-09T03:00:00.000Z" {
                // One made before the start and changed since, one made since. Linear lists newest first.
                json!([
                    recent_issue("2026-10-09T03:02:00.000Z", "2026-10-09T03:01:30.000Z"),
                    recent_issue("2026-10-09T03:01:00.000Z", "2026-10-01T00:00:00.000Z"),
                ])
            } else {
                json!([])
            };
            Reply::json(
                &json!({ "data": { "issues": { "nodes": nodes, "pageInfo": { "hasNextPage": false, "endCursor": null } } } }),
            )
        }
        _ => standard(call),
    });
    let linear = LinearTasks::connect_with(
        Config::new(KEY)
            .with_endpoint(&server.url)
            .with_poll_every(Duration::from_millis(60)),
    )
    .unwrap();
    let sub = linear.subscribe().unwrap();
    let wait = Duration::from_secs(3);
    let first = sub.recv_timeout(wait).expect("the change arrives");
    let second = sub.recv_timeout(wait).expect("the creation arrives");
    assert_eq!(
        (first.kind, first.task.version.as_str()),
        (EventKind::Updated, "2026-10-09T03:01:00.000Z"),
        "oldest change first"
    );
    assert_eq!(
        (second.kind, second.task.version.as_str()),
        (EventKind::Created, "2026-10-09T03:02:00.000Z")
    );
    // The next ask starts from the newest change it saw, so nothing arrives twice.
    thread::sleep(Duration::from_millis(250));
    assert!(sub.try_recv().is_err(), "nothing arrives twice");
    let since: Vec<_> = server
        .calls_of("List")
        .iter()
        .map(|c| c.variables["filter"]["and"][0]["updatedAt"]["gt"].clone())
        .collect();
    assert_eq!(since[0], "2026-10-09T03:00:00.000Z");
    assert_eq!(since[1], "2026-10-09T03:02:00.000Z");
    drop(sub);
    thread::sleep(Duration::from_millis(300));
    let asked = server.calls_of("List").len();
    thread::sleep(Duration::from_millis(400));
    assert_eq!(
        server.calls_of("List").len(),
        asked,
        "a dropped subscription stops asking"
    );
}
