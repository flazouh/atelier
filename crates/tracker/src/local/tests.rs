use std::{
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::Instant,
};

use super::{LocalTracker, migrations};
use crate::{
    ActivityKind, Assignee, Entry, Event, NewTask, Patch, PrLink, Priority, ProjectKey, Query, SessionLink, Status, TaskId, Tracker,
    TrackerError,
};

/// A tracker whose clock the test moves.
fn tracker() -> (LocalTracker, Arc<AtomicI64>) {
    let now = Arc::new(AtomicI64::new(1_000));
    let clock = now.clone();
    (LocalTracker::in_memory("lat").unwrap().with_clock(move || clock.load(Ordering::SeqCst)), now)
}

fn new(title: &str) -> NewTask {
    NewTask::titled(title)
}

#[test]
fn a_new_task_gets_the_next_short_id_and_reads_back() {
    let (t, _) = tracker();
    let a = t.create(&new("First"), "alex").unwrap();
    let b = t.create(&new("Second"), "alex").unwrap();
    assert_eq!((a.key.as_str(), b.key.as_str()), ("LAT-1", "LAT-2"), "the prefix is in capitals");
    assert_ne!(a.id, b.id);
    let mut full = new("  Third  ");
    full.description = "## Notes".into();
    full.status = Status::Backlog;
    full.priority = Priority::High;
    full.assignee = Some(Assignee::Agent("Claude".into()));
    full.labels = vec!["ui".into(), "bug".into(), "ui".into()];
    full.project = Some("lathe".into());
    full.parent = Some(a.id.clone());
    let c = t.create(&full, "alex").unwrap();
    assert_eq!(t.get(&c.id).unwrap().unwrap(), c);
    assert_eq!(c.title, "Third", "the title is trimmed");
    assert_eq!(c.labels, vec!["bug", "ui"], "labels are sorted and once each");
    assert_eq!((c.status, c.priority, c.created_at), (Status::Backlog, Priority::High, 1_000));
    assert_eq!(c.parent, Some(a.id));
}

#[test]
fn a_task_needs_a_title_and_a_real_parent() {
    let (t, _) = tracker();
    assert!(matches!(t.create(&new("   "), "a"), Err(TrackerError::Invalid(_))));
    let mut orphan = new("x");
    orphan.parent = Some(TaskId::from("99"));
    assert!(matches!(t.create(&orphan, "a"), Err(TrackerError::NotFound(_))));
    assert_eq!(t.list(&Query::default()).unwrap().len(), 0, "a refused task leaves nothing behind");
    assert_eq!(t.get(&TaskId::from("nope")).unwrap(), None);
    assert!(matches!(t.update(&TaskId::from("5"), &Patch::status(Status::Done), "a"), Err(TrackerError::NotFound(_))));
}

#[test]
fn creating_many_is_one_step_that_all_succeeds_or_none_does() {
    let (t, _) = tracker();
    let made = t.create_many(&[new("a"), new("b"), new("c")], "import").unwrap();
    assert_eq!(made.iter().map(|x| x.key.as_str()).collect::<Vec<_>>(), ["LAT-1", "LAT-2", "LAT-3"]);
    assert!(t.create_many(&[new("d"), new("")], "import").is_err());
    assert_eq!(t.list(&Query::default()).unwrap().len(), 3, "the good one of the failed batch was rolled back");
}

#[test]
fn the_log_starts_with_created_and_a_status_change_records_from_and_to() {
    let (t, now) = tracker();
    let task = t.create(&new("Do it"), "alex").unwrap();
    now.store(2_000, Ordering::SeqCst);
    let after = t.update(&task.id, &Patch::status(Status::InProgress), "alex").unwrap();
    assert_eq!((after.status, after.updated_at), (Status::InProgress, 2_000));
    let log = t.activity(&task.id).unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!((&log[0].kind, log[0].at, log[0].by.as_str()), (&ActivityKind::Created, 1_000, "alex"));
    assert_eq!(log[1].kind, ActivityKind::StatusChanged { from: Status::Todo, to: Status::InProgress });
    assert_eq!(log[1].at, 2_000);
    assert!(log[0].id < log[1].id, "the log is in the order it was written");
}

#[test]
fn a_patch_that_changes_nothing_writes_nothing() {
    let (t, now) = tracker();
    let task = t.create(&new("Same"), "a").unwrap();
    now.store(5_000, Ordering::SeqCst);
    let patch = Patch { title: Some("Same".into()), status: Some(Status::Todo), priority: Some(Priority::None), ..Patch::default() };
    let again = t.update(&task.id, &patch, "a").unwrap();
    assert_eq!(again.updated_at, 1_000, "not even the time moved");
    assert_eq!(t.activity(&task.id).unwrap().len(), 1);
    assert_eq!(t.update(&task.id, &Patch::default(), "a").unwrap(), task);
}

#[test]
fn assigning_logs_who_gets_it_and_taking_it_off_logs_nobody() {
    let (t, _) = tracker();
    let task = t.create(&new("Give"), "a").unwrap();
    let agent = Assignee::Agent("Claude".into());
    let given = t.update(&task.id, &Patch { assignee: Some(Some(agent.clone())), ..Patch::default() }, "alex").unwrap();
    assert_eq!(given.assignee, Some(agent.clone()));
    let taken = t.update(&task.id, &Patch { assignee: Some(None), ..Patch::default() }, "alex").unwrap();
    assert_eq!(taken.assignee, None);
    let kinds: Vec<_> = t.activity(&task.id).unwrap().into_iter().map(|a| a.kind).collect();
    assert_eq!(kinds[1], ActivityKind::Assigned { to: Some(agent) });
    assert_eq!(kinds[2], ActivityKind::Assigned { to: None });
}

#[test]
fn other_fields_change_and_log_an_edit_each() {
    let (t, _) = tracker();
    let task = t.create(&new("Old"), "a").unwrap();
    let patch = Patch {
        title: Some("New".into()),
        description: Some("Words".into()),
        priority: Some(Priority::Urgent),
        add_labels: vec!["b".into(), "a".into()],
        project: Some(Some("p".into())),
        ..Patch::default()
    };
    let after = t.update(&task.id, &patch, "a").unwrap();
    assert_eq!((after.title.as_str(), after.description.as_str(), after.priority), ("New", "Words", Priority::Urgent));
    assert_eq!((after.labels.clone(), after.project.clone()), (vec!["a".to_string(), "b".to_string()], Some("p".to_string())));
    let fields: Vec<_> = t
        .activity(&task.id)
        .unwrap()
        .into_iter()
        .filter_map(|a| if let ActivityKind::Edited { field } = a.kind { Some(field) } else { None })
        .collect();
    assert_eq!(fields, ["title", "description", "priority", "project", "labels"]);
    let removed = t.update(&task.id, &Patch { remove_labels: vec!["a".into()], ..Patch::default() }, "a").unwrap();
    assert_eq!(removed.labels, vec!["b"]);
    assert_eq!(t.labels().unwrap(), vec!["b"], "the labels in use");
    assert!(matches!(t.update(&task.id, &Patch { title: Some(" ".into()), ..Patch::default() }, "a"), Err(TrackerError::Invalid(_))));
}

#[test]
fn a_task_cannot_become_its_own_ancestor() {
    let (t, _) = tracker();
    let a = t.create(&new("a"), "x").unwrap();
    let mut child = new("b");
    child.parent = Some(a.id.clone());
    let b = t.create(&child, "x").unwrap();
    let mut grand = new("c");
    grand.parent = Some(b.id.clone());
    let c = t.create(&grand, "x").unwrap();
    let cycle = Patch { parent: Some(Some(c.id.clone())), ..Patch::default() };
    assert!(matches!(t.update(&a.id, &cycle, "x"), Err(TrackerError::Invalid(_))));
    assert!(matches!(t.update(&a.id, &Patch { parent: Some(Some(a.id.clone())), ..Patch::default() }, "x"), Err(TrackerError::Invalid(_))));
    assert_eq!(t.get(&a.id).unwrap().unwrap().parent, None, "nothing was written");
    let subs = t.list(&Query { parent: Some(a.id.clone()), ..Query::default() }).unwrap();
    assert_eq!(subs.iter().map(|s| s.id.clone()).collect::<Vec<_>>(), vec![b.id.clone()]);
    let free = t.update(&b.id, &Patch { parent: Some(None), ..Patch::default() }, "x").unwrap();
    assert_eq!(free.parent, None);
}

#[test]
fn comments_sessions_and_pull_requests_are_logged_and_linked() {
    let (t, now) = tracker();
    let task = t.create(&new("Ship"), "alex").unwrap();
    now.store(3_000, Ordering::SeqCst);
    let comment = t.record(&task.id, &Entry::Comment("  Looks good  ".into()), "alex").unwrap();
    assert_eq!(comment.kind, ActivityKind::Commented { text: "Looks good".into() });
    assert!(matches!(t.record(&task.id, &Entry::Comment("  ".into()), "alex"), Err(TrackerError::Invalid(_))));
    let session = SessionLink { session_id: "s1".into(), title: "Build".into(), agent: "Claude".into() };
    let pr = PrLink { number: 3344, repo: "o/r".into() };
    t.record(&task.id, &Entry::SessionStarted(session.clone()), "Claude").unwrap();
    t.record(&task.id, &Entry::SessionStarted(session.clone()), "Claude").unwrap();
    t.record(&task.id, &Entry::PrOpened(pr.clone()), "Claude").unwrap();
    t.record(&task.id, &Entry::PrMerged(pr.clone()), "alex").unwrap();
    let after = t.get(&task.id).unwrap().unwrap();
    assert_eq!((after.sessions, after.prs, after.updated_at), (vec![session], vec![pr], 3_000), "each link once");
    assert_eq!(t.tasks_of_session("s1").unwrap(), vec![task.id.clone()]);
    assert_eq!(t.tasks_of_pr(3344).unwrap(), vec![task.id.clone()]);
    assert!(t.tasks_of_pr(1).unwrap().is_empty() && t.tasks_of_session("zz").unwrap().is_empty());
    assert!(matches!(t.record(&TaskId::from("42"), &Entry::Comment("x".into()), "a"), Err(TrackerError::NotFound(_))));
    assert_eq!(t.activity(&task.id).unwrap().len(), 1 + 1 + 2 + 1 + 1, "created, comment, two session starts, opened, merged");
}

fn fill(t: &LocalTracker) {
    let mut a = new("Fix the sidebar");
    a.status = Status::InProgress;
    a.priority = Priority::High;
    a.assignee = Some(Assignee::Person("Sam".into()));
    a.labels = vec!["ui".into()];
    let mut b = new("Port the forge_client");
    b.priority = Priority::Low;
    b.assignee = Some(Assignee::Agent("Claude".into()));
    b.labels = vec!["ui".into(), "agents".into()];
    let mut c = new("100% done");
    c.status = Status::Done;
    t.create_many(&[a, b, c], "alex").unwrap();
}

fn titles(tasks: Vec<crate::Task>) -> Vec<String> {
    tasks.into_iter().map(|t| t.title).collect()
}

#[test]
fn a_query_narrows_by_every_part_it_sets() {
    let (t, _) = tracker();
    fill(&t);
    let list = |q: Query| titles(t.list(&q).unwrap());
    assert_eq!(list(Query::default()).len(), 3);
    assert_eq!(list(Query { statuses: vec![Status::InProgress, Status::Done], ..Query::default() }).len(), 2);
    assert_eq!(list(Query { assignee: Some("Claude".into()), ..Query::default() }), ["Port the forge_client"]);
    assert_eq!(list(Query { label: Some("ui".into()), ..Query::default() }).len(), 2);
    assert_eq!(list(Query { label: Some("ui".into()), priority: Some(Priority::High), ..Query::default() }), ["Fix the sidebar"]);
    assert_eq!(list(Query { text: Some("SIDEBAR".into()), ..Query::default() }), ["Fix the sidebar"], "the case does not matter");
    assert_eq!(list(Query { text: Some("lat-3".into()), ..Query::default() }), ["100% done"], "the short id works too");
    assert_eq!(list(Query { text: Some("_".into()), ..Query::default() }), ["Port the forge_client"], "an underscore is a character, not a wildcard");
    assert_eq!(list(Query { text: Some("100%".into()), ..Query::default() }), ["100% done"]);
    assert!(list(Query { text: Some("zzz".into()), ..Query::default() }).is_empty());
}

#[test]
fn the_list_is_the_most_recently_changed_first_and_carries_its_links() {
    let (t, now) = tracker();
    fill(&t);
    now.store(9_000, Ordering::SeqCst);
    let first = t.list(&Query::default()).unwrap().pop().unwrap();
    t.record(&first.id, &Entry::PrOpened(PrLink { number: 5, repo: "o/r".into() }), "x").unwrap();
    let list = t.list(&Query::default()).unwrap();
    assert_eq!(list[0].id, first.id, "the task that changed last is first");
    assert_eq!(list[0].prs.len(), 1);
    assert_eq!(list.iter().find(|x| x.title == "Port the forge_client").unwrap().labels, vec!["agents", "ui"]);
}

#[test]
fn subscribers_hear_of_each_change_and_a_dropped_receiver_is_forgotten() {
    let (t, _) = tracker();
    let events = t.subscribe();
    let gone = t.subscribe();
    drop(gone);
    let task = t.create(&new("Watch"), "a").unwrap();
    t.update(&task.id, &Patch::status(Status::Done), "a").unwrap();
    t.record(&task.id, &Entry::Comment("hi".into()), "a").unwrap();
    let heard: Vec<_> = events.try_iter().collect();
    assert!(matches!(&heard[0], Event::Created(x) if x.id == task.id));
    assert!(matches!(&heard[1], Event::Updated(x) if x.status == Status::Done));
    assert!(matches!(&heard[2], Event::Activity(a) if a.kind == ActivityKind::StatusChanged { from: Status::Todo, to: Status::Done }));
    assert!(matches!(&heard[4], Event::Activity(a) if matches!(a.kind, ActivityKind::Commented { .. })));
    assert_eq!(heard.len(), 5);
    assert_eq!(t.subscribers.lock().unwrap().len(), 1);
}

#[test]
fn the_database_survives_a_reopen_and_keeps_its_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let project = ProjectKey::Local { path: "/code/lathe".into() };
    let id = {
        let t = LocalTracker::open_project(dir.path(), &project, "lathe").unwrap();
        assert_eq!(t.prefix(), "LAT");
        t.create(&new("Kept"), "a").unwrap().id
    };
    assert!(project.path_in(dir.path()).exists(), "the database is in the data folder");
    let t = LocalTracker::open_project(dir.path(), &project, "renamed-project").unwrap();
    assert_eq!(t.prefix(), "LAT", "a renamed project keeps its short ids");
    let task = t.get(&id).unwrap().unwrap();
    assert_eq!((task.title.as_str(), task.key.as_str()), ("Kept", "LAT-1"));
    assert_eq!(t.create(&new("Next"), "a").unwrap().key, "LAT-2");
}

#[test]
fn two_projects_keep_two_databases() {
    let dir = tempfile::tempdir().unwrap();
    let a = LocalTracker::open_project(dir.path(), &ProjectKey::Local { path: "/a/app".into() }, "app").unwrap();
    let b = LocalTracker::open_project(dir.path(), &ProjectKey::Ssh { host: "hp".into(), path: "/a/app".into() }, "app").unwrap();
    a.create(&new("Only in a"), "x").unwrap();
    assert!(b.list(&Query::default()).unwrap().is_empty());
}

#[test]
fn an_old_database_is_brought_up_and_a_newer_one_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.sqlite");
    {
        let t = LocalTracker::open(&path, "T").unwrap();
        t.create(&new("Old"), "a").unwrap();
    }
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        let version: usize = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(version, migrations::CURRENT);
        conn.pragma_update(None, "user_version", migrations::CURRENT + 1).unwrap();
    }
    let error = LocalTracker::open(&path, "T").err().expect("a newer database is refused");
    assert!(matches!(&error, TrackerError::Storage(why) if why.contains("update lathe")), "{error}");
}

/// The numbers the task asks for. Run in release: `cargo test --release -p lathe-tracker -- --ignored --nocapture`.
#[test]
#[ignore = "a measuring run; needs --release"]
fn ten_thousand_tasks_load_and_filter_in_under_50_ms_and_a_write_takes_under_5() {
    // A file, not memory: a write has to reach the log on disk.
    let dir = tempfile::tempdir().unwrap();
    let t = LocalTracker::open(&dir.path().join("perf.sqlite"), "LAT").unwrap();
    let labels = ["ui", "bug", "perf", "docs", "infra", "agents", "review"];
    let batch: Vec<NewTask> = (0..10_000)
        .map(|i| {
            let mut task = new(&format!("Task number {i} about the {}", labels[i % labels.len()]));
            task.status = Status::ALL[i % 6];
            task.priority = Priority::ALL[i % 5];
            task.assignee = (i % 3 != 0).then(|| if i % 2 == 0 { Assignee::Agent("Claude".into()) } else { Assignee::Person("Sam".into()) });
            task.labels = vec![labels[i % 7].into(), labels[(i / 7) % 7].into()];
            task
        })
        .collect();
    let started = Instant::now();
    t.create_many(&batch, "import").unwrap();
    println!("import of 10,000 in one step: {:?}", started.elapsed());
    let mut load = Vec::new();
    let mut filter = Vec::new();
    for _ in 0..20 {
        let started = Instant::now();
        assert_eq!(t.list(&Query::default()).unwrap().len(), 10_000);
        load.push(started.elapsed());
        let started = Instant::now();
        let some = t.list(&Query { label: Some("ui".into()), priority: Some(Priority::High), text: Some("task".into()), ..Query::default() }).unwrap();
        filter.push(started.elapsed());
        assert!(!some.is_empty());
    }
    let mut writes = Vec::new();
    for i in 0..200 {
        let id = TaskId((i * 37 % 10_000 + 1).to_string());
        let status = if i % 2 == 0 { Status::Done } else { Status::InProgress };
        let started = Instant::now();
        t.update(&id, &Patch::status(status), "alex").unwrap();
        writes.push(started.elapsed());
    }
    let median = |v: &mut Vec<std::time::Duration>| {
        v.sort();
        (v[v.len() / 2], v[v.len() * 95 / 100], *v.last().unwrap())
    };
    let (load, filter, writes) = (median(&mut load), median(&mut filter), median(&mut writes));
    println!("load all 10,000:          median {:?}  p95 {:?}  max {:?}", load.0, load.1, load.2);
    println!("filter (label+prio+text): median {:?}  p95 {:?}  max {:?}", filter.0, filter.1, filter.2);
    println!("one status write:         median {:?}  p95 {:?}  max {:?}", writes.0, writes.1, writes.2);
    assert!(load.0.as_millis() < 50 && filter.0.as_millis() < 50, "load and filter under 50 ms");
    assert!(writes.0.as_millis() < 5, "a write under 5 ms");
}

#[test]
fn a_commit_is_a_line_of_the_activity_and_links_nothing_new() {
    let t = LocalTracker::in_memory("LAT").unwrap();
    let task = t.create(&NewTask::titled("Fix it"), "me").unwrap();
    let line = t.record(&task.id, &Entry::Commit { sha: "abc1234".into(), subject: "Fix the scroll".into() }, "me").unwrap();
    assert_eq!(line.kind, ActivityKind::Commit { sha: "abc1234".into(), subject: "Fix the scroll".into() });
    let log = t.activity(&task.id).unwrap();
    assert_eq!(log.last().map(|a| &a.kind), Some(&ActivityKind::Commit { sha: "abc1234".into(), subject: "Fix the scroll".into() }));
    let after = t.get(&task.id).unwrap().unwrap();
    assert!(after.sessions.is_empty() && after.prs.is_empty(), "a commit adds no link");
}
