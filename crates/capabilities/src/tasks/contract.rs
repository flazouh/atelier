//! The contract every tasks provider passes. A provider's crate calls [`run`] from one test with a function that makes a fresh,
//! empty provider. Each check panics with a plain message when the provider breaks it.
//!
//! The nine checks are the ones in `docs/capabilities/tasks-v1.md`, section 9.
use super::{
    structs::{NewTask, Patch, Query},
    traits::TasksProvider,
    types::{Change, EventKind, Priority},
};
use crate::{Actor, CapError, Capabilities, Operation, Ref};

/// Makes a fresh provider with no tasks.
pub type Make<'a> = &'a dyn Fn() -> Box<dyn TasksProvider>;

pub fn run(make: Make) {
    create_then_get(make);
    patch_changes_only_what_it_names(make);
    a_stale_version_conflicts(make);
    pages_do_not_repeat_or_skip(make);
    capabilities_are_honest(make);
    export_then_import_moves_everything(make);
    subscribe_delivers_each_change_once_in_order(make);
    the_actor_is_kept(make);
    a_reference_parses_back(make);
}

fn alex() -> Actor {
    Actor::person("alex", "Alex")
}

fn new(title: &str) -> NewTask {
    NewTask::titled(title)
}

/// 1. `create` then `get` returns what was sent.
pub fn create_then_get(make: Make) {
    let p = make();
    let mut sent = new("Fix the login");
    sent.description = "It fails on Safari.".into();
    sent.priority = Priority::High;
    let made = p.create(&sent, &alex()).expect("create");
    let read = p.get(&made.reference).expect("get what create made");
    assert_eq!(read.title, "Fix the login", "the title");
    assert_eq!(read.description, "It fails on Safari.", "the description");
    assert_eq!(read.priority, Priority::High, "the priority");
    assert_eq!(read, made, "get returns what create returned");
    assert!(
        matches!(
            p.create(&new("   "), &alex()),
            Err(CapError::Invalid { .. })
        ),
        "an empty title is invalid"
    );
    let missing: Ref = format!("tasks:{}:{}:NOPE-9", p.provider(), p.account())
        .parse()
        .unwrap();
    assert!(
        matches!(p.get(&missing), Err(CapError::NotFound { .. })),
        "an unknown task is not found"
    );
}

/// 2. `update` changes only the fields in the patch, and a cleared field is cleared.
pub fn patch_changes_only_what_it_names(make: Make) {
    let p = make();
    let t = p
        .create(
            &NewTask {
                description: "notes".into(),
                priority: Priority::Low,
                ..new("Title")
            },
            &alex(),
        )
        .unwrap();
    let after = p
        .update(
            &t.reference,
            &Patch {
                title: Some("New title".into()),
                ..Patch::default()
            },
            &t.version,
            &alex(),
        )
        .expect("update");
    assert_eq!(
        (
            after.title.as_str(),
            after.description.as_str(),
            after.priority
        ),
        ("New title", "notes", Priority::Low),
        "only the title changed"
    );
    let cleared = p
        .update(
            &t.reference,
            &Patch {
                description: Change::Clear,
                ..Patch::default()
            },
            &after.version,
            &alex(),
        )
        .expect("clear");
    assert_eq!(cleared.description, "", "a cleared description is empty");
    assert_eq!(cleared.title, "New title", "the title stayed");
    let same = p
        .update(&t.reference, &Patch::default(), &cleared.version, &alex())
        .expect("an empty patch");
    assert_eq!(
        same.version, cleared.version,
        "a patch that changes nothing keeps the version"
    );
}

/// 3. An `update` with an old `version` is a `Conflict` that carries the task as it is.
pub fn a_stale_version_conflicts(make: Make) {
    let p = make();
    let t = p.create(&new("Race"), &alex()).unwrap();
    let first = p
        .update(
            &t.reference,
            &Patch {
                title: Some("First".into()),
                ..Patch::default()
            },
            &t.version,
            &alex(),
        )
        .unwrap();
    let second = p.update(
        &t.reference,
        &Patch {
            title: Some("Second".into()),
            ..Patch::default()
        },
        &t.version,
        &alex(),
    );
    match second {
        Err(CapError::Conflict { current }) => assert_eq!(
            current["title"], "First",
            "the conflict carries the current task"
        ),
        other => panic!("a stale version must conflict, got {other:?}"),
    }
    assert_eq!(
        p.get(&t.reference).unwrap().version,
        first.version,
        "the stale write changed nothing"
    );
}

/// 4. A list is stable across pages: no task twice, none missed, while the data does not change.
pub fn pages_do_not_repeat_or_skip(make: Make) {
    let p = make();
    let mut made: Vec<String> = (0..7)
        .map(|i| p.create(&new(&format!("Task {i}")), &alex()).unwrap().key)
        .collect();
    let mut seen = Vec::new();
    let mut cursor = None;
    for _ in 0..20 {
        let page = p
            .list(&Query {
                limit: Some(3),
                cursor: cursor.clone(),
                ..Query::default()
            })
            .expect("list a page");
        assert!(page.items.len() <= 3, "a page keeps to its limit");
        seen.extend(page.items.into_iter().map(|t| t.key));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert!(cursor.is_none(), "the pages end");
    seen.sort();
    made.sort();
    assert_eq!(seen, made, "every task once");
}

/// 5. `capabilities` is honest: the core calls are listed, and an optional call is listed exactly when it works.
pub fn capabilities_are_honest(make: Make) {
    let p = make();
    let caps: Capabilities = p.capabilities();
    for op in Capabilities::CORE {
        assert!(caps.can(op), "a provider lists the core call {op:?}");
    }
    let t = p.create(&new("Probe"), &alex()).unwrap();
    let unsupported = |r: Result<(), CapError>| matches!(r, Err(CapError::Unsupported { .. }));
    assert_eq!(
        !unsupported(p.statuses().map(|_| ())),
        caps.can(Operation::Statuses),
        "statuses is listed exactly when it works"
    );
    assert_eq!(
        !unsupported(p.export(None).map(|_| ())),
        caps.can(Operation::Export),
        "export is listed exactly when it works"
    );
    assert_eq!(
        !unsupported(p.import(&[]).map(|_| ())),
        caps.can(Operation::Import),
        "import is listed exactly when it works"
    );
    if !caps.can(Operation::Delete) {
        assert!(
            unsupported(p.delete(&t.reference, &alex())),
            "delete is not listed, so it is unsupported"
        );
    }
}

/// 6. `export` then `import` into an empty provider gives the same tasks, and a second `import` changes nothing.
pub fn export_then_import_moves_everything(make: Make) {
    let from = make();
    if !(from.can(Operation::Export) && from.can(Operation::Import)) {
        return;
    }
    let a = from
        .create(
            &NewTask {
                description: "one".into(),
                priority: Priority::Urgent,
                ..new("One")
            },
            &alex(),
        )
        .unwrap();
    let b = from.create(&new("Two"), &alex()).unwrap();
    from.comment(
        &a.reference,
        "a comment",
        &Actor::agent("agent-1", "Claude", "alex"),
    )
    .unwrap();
    from.update(
        &b.reference,
        &Patch {
            priority: Some(Priority::Low),
            ..Patch::default()
        },
        &b.version,
        &alex(),
    )
    .unwrap();
    let mut batch = Vec::new();
    let mut cursor = None;
    loop {
        let page = from.export(cursor.as_deref()).expect("export");
        batch.extend(page.items);
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    let to = make();
    assert!(
        to.import(&batch).expect("import") > 0,
        "the first import writes"
    );
    let all = |p: &dyn TasksProvider| {
        let mut tasks = p
            .list(&Query {
                limit: Some(100),
                ..Query::default()
            })
            .unwrap()
            .items;
        tasks.sort_by(|x, y| x.key.cmp(&y.key));
        tasks
    };
    assert_eq!(
        all(to.as_ref()),
        all(from.as_ref()),
        "the same tasks arrive"
    );
    assert_eq!(
        to.activity(&a.reference, None).unwrap().items,
        from.activity(&a.reference, None).unwrap().items,
        "the activity arrives"
    );
    let before = all(to.as_ref());
    assert_eq!(
        to.import(&batch).expect("import again"),
        0,
        "the second import writes nothing"
    );
    assert_eq!(
        all(to.as_ref()),
        before,
        "the second import changes nothing"
    );
}

/// 7. `subscribe` delivers each change once, in order. A provider may also tell the activity of a change as its own event.
pub fn subscribe_delivers_each_change_once_in_order(make: Make) {
    let p = make();
    let sub = p.subscribe().expect("subscribe");
    let t = p.create(&new("Watched"), &alex()).unwrap();
    p.update(
        &t.reference,
        &Patch {
            title: Some("Renamed".into()),
            ..Patch::default()
        },
        &t.version,
        &alex(),
    )
    .unwrap();
    let mut changes = Vec::new();
    let mut quiet = std::time::Duration::from_secs(2);
    while let Ok(event) = sub.recv_timeout(quiet) {
        quiet = std::time::Duration::from_millis(300);
        if event.kind != EventKind::Activity {
            changes.push((event.kind, event.task.title));
        }
    }
    assert_eq!(
        changes,
        [
            (EventKind::Created, "Watched".to_string()),
            (EventKind::Updated, "Renamed".to_string())
        ],
        "each change arrives once, in order"
    );
}

/// 8. The actor is kept, with who it works for.
pub fn the_actor_is_kept(make: Make) {
    let p = make();
    let t = p.create(&new("Who"), &alex()).unwrap();
    let agent = Actor::agent("agent-1", "Claude", "alex");
    let c = p
        .comment(&t.reference, "From an agent", &agent)
        .expect("comment");
    assert_eq!(
        c.author, agent,
        "the comment keeps the agent and its person"
    );
    let log = p.activity(&t.reference, None).unwrap().items;
    let commented = log
        .iter()
        .find(|a| a.kind == super::types::ActivityKind::Commented)
        .expect("the comment is in the activity");
    assert_eq!(
        commented.by.on_behalf_of.as_deref(),
        Some("alex"),
        "the activity keeps on_behalf_of"
    );
}

/// 9. A reference parses back to its provider and id.
pub fn a_reference_parses_back(make: Make) {
    let p = make();
    let t = p.create(&new("Ref"), &alex()).unwrap();
    let again: Ref = t
        .reference
        .to_string()
        .parse()
        .expect("the reference parses");
    assert_eq!(again, t.reference);
    assert_eq!(
        (
            again.capability.as_str(),
            again.provider.as_str(),
            again.account.as_str()
        ),
        ("tasks", p.provider(), p.account())
    );
}
